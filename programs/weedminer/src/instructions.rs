use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer},
};

use crate::{constants::*, errors::WeedMinerError, helpers::*, state::*};
use switchboard_on_demand::accounts::RandomnessAccountData;

/// ────────────────────────────────────────────────────────────────────────────
/// INTERNAL: update the global accumulator
/// ────────────────────────────────────────────────────────────────────────────
fn update_pool(gs: &mut GlobalState, slot_now: u64) {
    if slot_now <= gs.last_reward_slot || gs.total_hashpower == 0 {
        gs.last_reward_slot = slot_now;
        return;
    }
    // Calculate theoretical halvings based on elapsed slots
    let raw_halvings = calculate_halvings(slot_now, gs.start_slot, gs.halving_interval);

    // Limit halvings to the maximum meaningful value
    let max_halvings = calculate_max_halvings(gs.initial_reward_rate);
    let halvings = raw_halvings.min(max_halvings);

    let rate_now = reward_after_halvings(gs.initial_reward_rate, halvings);

    /* remaining supply after accounting for burns */
    let minted_minus_burn = gs.cumulative_rewards.saturating_sub(gs.burned_tokens);
    let remaining_supply = gs.total_supply.saturating_sub(minted_minus_burn);

    let dust_threshold = if gs.dust_threshold_divisor > 0 {
        gs.total_supply / gs.dust_threshold_divisor
    } else {
        0 // Avoid division by zero, effectively disabling dust threshold if misconfigured
    };
    // Check if we're close to depleting the supply
    if remaining_supply <= dust_threshold || rate_now == 0 {
        // Then set rate to zero to prevent future mining
        gs.current_reward_rate = 0;
        gs.last_reward_slot = slot_now;
        return;
    }

    let slots_elapsed = (slot_now - gs.last_reward_slot) as u128;
    let mut reward = slots_elapsed
        .checked_mul(rate_now as u128)
        .unwrap_or(u128::MAX);
    reward = reward.min(remaining_supply as u128); // clamp to cap

    gs.acc_bits_per_hash += reward * ACC_SCALE / gs.total_hashpower as u128;
    gs.cumulative_rewards = gs.cumulative_rewards.saturating_add(reward as u64);

    gs.current_reward_rate = if remaining_supply > 0 { rate_now } else { 0 };

    gs.last_reward_slot = slot_now;
    gs.last_processed_halvings = halvings;
}

/// Helper to settle and mint rewards for a player.
/// Returns Ok(amount_claimed) or Ok(0) if nothing to claim.
fn settle_and_mint_rewards<'info>(
    player: &mut Account<'info, Player>,
    gs: &mut Account<'info, GlobalState>,
    now: u64,
    player_token_account: &AccountInfo<'info>,
    referrer_token_account: Option<&AccountInfo<'info>>,
    fees_token_account: &AccountInfo<'info>,
    token_mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    global_state_bump: u8,
) -> Result<u64> {
    // update pool to now
    update_pool(gs, now);

    require!(
        now > player.last_claim_slot,
        WeedMinerError::CooldownNotExpired
    );

    // calculate pending
    let pending_u128 = (player.hashpower as u128)
        .checked_mul(
            gs.acc_bits_per_hash
                .saturating_sub(player.last_acc_bits_per_hash),
        )
        .unwrap_or(u128::MAX)
        / ACC_SCALE;
    let mut pending = pending_u128 as u64;

    // Clamp pending to remaining supply
    let minted_minus_burn = gs.cumulative_rewards.saturating_sub(gs.burned_tokens);
    let remaining_supply = gs.total_supply.saturating_sub(minted_minus_burn);
    if pending > remaining_supply {
        pending = remaining_supply;
    }

    if pending == 0 {
        player.last_claim_slot = now;
        player.last_acc_bits_per_hash = gs.acc_bits_per_hash;
        return Ok(0);
    }

    // update player bookkeeping
    player.last_claim_slot = now;
    player.last_acc_bits_per_hash = gs.acc_bits_per_hash;

    // split referral based on global_state.referral_fee
    let referral_amount = pending * gs.referral_fee as u64 / 100;
    let player_amount = pending - referral_amount;

    // Update player total rewards (Effect)
    player.total_rewards = player.total_rewards.saturating_add(player_amount);

    // signer seeds
    let token_mint_key = &token_mint.key();
    let seeds = &[
        GLOBAL_STATE_SEED,
        token_mint_key.as_ref(),
        &[global_state_bump],
    ];
    let signer = &[&seeds[..]];

    // mint to player
    token::mint_to(
        CpiContext::new_with_signer(
            token_program.clone(),
            MintTo {
                mint: token_mint.clone(),
                to: player_token_account.clone(),
                authority: gs.to_account_info(),
            },
            signer,
        ),
        player_amount,
    )?;

    // referral / governance
    if let Some(referrer_account) = referrer_token_account {
        token::mint_to(
            CpiContext::new_with_signer(
                token_program.clone(),
                MintTo {
                    mint: token_mint.clone(),
                    to: referrer_account.clone(),
                    authority: gs.to_account_info(),
                },
                signer,
            ),
            referral_amount,
        )?;
    } else {
        token::mint_to(
            CpiContext::new_with_signer(
                token_program.clone(),
                MintTo {
                    mint: token_mint.clone(),
                    to: fees_token_account.clone(),
                    authority: gs.to_account_info(),
                },
                signer,
            ),
            referral_amount,
        )?;
    }

    Ok(pending)
}

/// ────────────────────────────────────────────────────────────────────────────
/* ──────────────────────────
INITIALIZE
────────────────────────── */
#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8  /* discriminator */
        + 32 + 32 + 32          /* authority + mint + fees_wallet */
        + 8  + 8                /* total_supply + burned_tokens */
        + 8  + 8                /* cumulative_rewards + start_slot */
        + 8  + 8  + 8           /* halving_interval + last_halvings + initial_rate */
        + 8  + 16 + 8           /* current_rate + acc_bits_per_hash (u128!) + last_reward_slot */
        + 1  + 1 + 1 + 8 + 8    /* burn_rate + referral_fee + prod + cooldown + dust_divisor */
        + 8                     /* total_hashpower */
        + 33                    /* global_random_reward: Option<GlobalRandomReward> (1 + 8 + 8 + 8 + 8) */
        + 8                     /* global_reward_counter */
        + 8 + 8,                /* total_global_gambles + total_global_gamble_wins */
        seeds=[GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,
    /// CHECK: This is the fees recipient wallet
    pub fees_wallet: AccountInfo<'info>,
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = fees_wallet,
    )]
    pub fees_token_account: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn initialize_program(
    ctx: Context<InitializeProgram>,
    start_slot: u64,
    halving_interval: u64,
    total_supply: u64,
    initial_reward_rate: u64,
    cooldown_slots: Option<u64>,
) -> Result<()> {
    let gs = &mut ctx.accounts.global_state;

    gs.authority = ctx.accounts.authority.key();
    gs.token_mint = ctx.accounts.token_mint.key();
    gs.fees_wallet = ctx.accounts.fees_wallet.key();

    gs.total_supply = total_supply;
    gs.burned_tokens = 0;
    gs.cumulative_rewards = 0;

    gs.start_slot = start_slot;
    gs.halving_interval = halving_interval;
    gs.last_processed_halvings = 0;
    gs.initial_reward_rate = initial_reward_rate;
    gs.current_reward_rate = initial_reward_rate;

    gs.acc_bits_per_hash = 0;
    gs.last_reward_slot = start_slot;

    gs.burn_rate = 75;
    gs.referral_fee = 25;
    gs.production_enabled = true;
    gs.cooldown_slots = cooldown_slots.unwrap_or(108_000); // 12 hours
    gs.dust_threshold_divisor = 1000; // Default to 0.1%

    gs.total_hashpower = 0;
    gs.global_random_reward = None;
    gs.global_reward_counter = 0;

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  PURCHASE INITIAL FACILITY
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(referrer: Option<Pubkey>)]
pub struct PurchaseInitialFacility<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        init,
        payer = player_wallet,
        space = 8      // discriminator
            + 32       // owner
            + 10       // facility
            + 4        // machines vec header
            + (12 * 17)// MAX_MINERS (12) * size_of(Machine)
            + 8        // hashpower
            + 33       // referrer
            + 16       // last_acc_bits_per_hash
            + 8        // last_claim_slot
            + 8        // last_upgrade_slot
            + 8        // total_rewards
            + 8        // last_claimed_global_reward_id
            + 8 + 8    // total_gambles + total_gamble_wins
            + 32       // randomness_account
            + 8        // commit_slot
            + 8        // current_gamble_amount
            + 1,       // has_pending_gamble
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    /// CHECK: This is the fees recipient wallet from global_state
    #[account(
        mut,
        constraint = fees_wallet.key() == global_state.fees_wallet @ WeedMinerError::Unauthorized
    )]
    pub fees_wallet: AccountInfo<'info>,
    #[account(
        mut,
        constraint = token_mint.key() == global_state.token_mint @ WeedMinerError::InvalidTokenMint
    )]
    pub token_mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = player_wallet,
        associated_token::mint = token_mint,
        associated_token::authority = player_wallet,
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[event]
pub struct InitialFacilityPurchased {
    pub player_wallet: Pubkey,
    pub player_account: Pubkey,
    pub referrer: Option<Pubkey>,
    pub facility_type: u8,
    pub initial_machines: u8,
    pub initial_hashpower: u64,
    pub slot: u64,
}

pub fn purchase_initial_facility(
    ctx: Context<PurchaseInitialFacility>,
    referrer: Option<Pubkey>,
) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(gs.production_enabled, WeedMinerError::ProductionDisabled);
    require!(
        player.machines.is_empty(),
        WeedMinerError::InitialFacilityAlreadyPurchased
    );

    // Prevent self-referral
    if let Some(ref r) = referrer {
        require!(
            *r != ctx.accounts.player_wallet.key(),
            WeedMinerError::SelfReferralNotAllowed
        );
    }

    // Make sure pool is up to date
    update_pool(gs, slot);

    // transfer 1 SOL to fees wallet
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.player_wallet.to_account_info(),
                to: ctx.accounts.fees_wallet.to_account_info(),
            },
        ),
        250_000_000,
    )?;

    // player bootstrap
    player.owner = ctx.accounts.player_wallet.key();
    player.facility = Facility {
        facility_type: 0,  // Starter Shack
        total_machines: 2, // From facilities.json
        power_output: 15,  // From facilities.json
    };
    player.machines = vec![Machine {
        machine_type: 0,      // Nano Rig
        hashrate: 1_500,      // From machines.json
        power_consumption: 3, // From machines.json
    }];
    player.hashpower = 1_500; // Match the initial miner's hashrate
    player.referrer = referrer;
    player.last_claim_slot = slot;
    player.last_upgrade_slot = slot;
    player.total_rewards = 0;
    player.last_acc_bits_per_hash = gs.acc_bits_per_hash;
    player.last_claimed_global_reward_id = 0;

    // Initialize gambling fields
    player.total_gambles = 0;
    player.total_gamble_wins = 0;

    // Initialize Switchboard randomness fields
    player.randomness_account = Pubkey::default();
    player.commit_slot = 0;
    player.current_gamble_amount = 0;
    player.has_pending_gamble = false;

    // global stats (Effect)
    gs.total_hashpower += 1_500;

    emit!(InitialFacilityPurchased {
        player_wallet: ctx.accounts.player_wallet.key(),
        player_account: player.key(),
        referrer,
        facility_type: player.facility.facility_type,
        initial_machines: player.machines.len() as u8,
        initial_hashpower: player.hashpower,
        slot,
    });

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  BUY MINER
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(machine_type: u8)]
pub struct BuyMachine<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ WeedMinerError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        constraint = player_token_account.mint == global_state.token_mint
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = fees_token_account.mint == global_state.token_mint,
        constraint = fees_token_account.owner == global_state.fees_wallet @ WeedMinerError::Unauthorized
    )]
    pub fees_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn buy_machine(ctx: Context<BuyMachine>, machine_type: u8) -> Result<()> {
    require!(
        machine_type < MACHINE_CONFIGS.len() as u8,
        WeedMinerError::InvalidMachineType
    );

    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // guards
    require!(gs.production_enabled, WeedMinerError::ProductionDisabled);

    require!(
        player.machines.len() < player.facility.total_machines as usize,
        WeedMinerError::MachineCapacityExceeded
    );

    settle_and_mint_rewards(
        player,
        gs,
        slot,
        &ctx.accounts.player_token_account.to_account_info(),
        None,
        &ctx.accounts.fees_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    // configs
    let (hashrate, power_consumption, cost) = MACHINE_CONFIGS[machine_type as usize];
    let total_power = player
        .machines
        .iter()
        .map(|m| m.power_consumption)
        .sum::<u64>()
        + power_consumption;
    require!(
        total_power <= player.facility.power_output,
        WeedMinerError::PowerCapacityExceeded
    );
    require!(
        ctx.accounts.player_token_account.amount >= cost,
        WeedMinerError::InsufficientBits
    );

    // Calculate amounts for burn/transfer
    let burn_amount = cost * gs.burn_rate as u64 / 100;
    let fees_amount = cost - burn_amount;

    // === EFFECTS ===
    // Update global state for burned tokens
    gs.burned_tokens = gs.burned_tokens.saturating_add(burn_amount);

    // Add new miner and update player & global hashpower
    player.machines.push(Machine {
        machine_type,
        hashrate,
        power_consumption,
    });
    player.hashpower += hashrate;
    gs.total_hashpower += hashrate;

    // Update player's accumulator state
    player.last_acc_bits_per_hash = gs.acc_bits_per_hash;

    // === INTERACTIONS ===
    // Burn BITS
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.token_mint.to_account_info(),
                from: ctx.accounts.player_token_account.to_account_info(),
                authority: ctx.accounts.player_wallet.to_account_info(),
            },
        ),
        burn_amount,
    )?;

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.player_token_account.to_account_info(),
                to: ctx.accounts.fees_token_account.to_account_info(),
                authority: ctx.accounts.player_wallet.to_account_info(),
            },
        ),
        fees_amount,
    )?;

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  SELL MINER
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(machine_index: u8)]
pub struct SellMachine<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ WeedMinerError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        constraint = player_token_account.mint == global_state.token_mint
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = fees_token_account.mint == global_state.token_mint,
        constraint = fees_token_account.owner == global_state.fees_wallet @ WeedMinerError::Unauthorized
    )]
    pub fees_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

pub fn sell_machine(ctx: Context<SellMachine>, machine_index: u8) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(gs.production_enabled, WeedMinerError::ProductionDisabled);

    require!(
        machine_index < player.machines.len() as u8,
        WeedMinerError::InvalidMachineType
    );

    settle_and_mint_rewards(
        player,
        gs,
        slot,
        &ctx.accounts.player_token_account.to_account_info(),
        None,
        &ctx.accounts.fees_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    let miner = player.machines.remove(machine_index as usize);

    player.hashpower = player.hashpower.saturating_sub(miner.hashrate);
    gs.total_hashpower = gs.total_hashpower.saturating_sub(miner.hashrate);

    player.last_acc_bits_per_hash = gs.acc_bits_per_hash;

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  UPGRADE FACILITY  (hp does not change → only update pool/debt if you add hp)
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(facility_type: u8)]
pub struct UpgradeFacility<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ WeedMinerError::Unauthorized,
        constraint = facility_type > player.facility.facility_type @ WeedMinerError::InvalidFacilityType,
        constraint = facility_type <= HIGH_RISE_APARTMENT @ WeedMinerError::InvalidFacilityType,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        constraint = player_token_account.mint == global_state.token_mint
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = fees_token_account.mint == global_state.token_mint,
        constraint = fees_token_account.owner == global_state.fees_wallet @ WeedMinerError::Unauthorized
    )]
    pub fees_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

pub fn upgrade_facility(ctx: Context<UpgradeFacility>, facility_type: u8) -> Result<()> {
    require!(
        (LOW_PROFILE_STORAGE..=HIGH_RISE_APARTMENT).contains(&facility_type),
        WeedMinerError::InvalidFacilityType
    );

    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    update_pool(gs, slot);

    require!(gs.production_enabled, WeedMinerError::ProductionDisabled);
    require!(
        slot >= player.last_upgrade_slot + gs.cooldown_slots,
        WeedMinerError::CooldownNotExpired
    );

    settle_and_mint_rewards(
        player,
        gs,
        slot,
        &ctx.accounts.player_token_account.to_account_info(),
        None,
        &ctx.accounts.fees_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    let (total_machines, power_output, cost) = FACILITY_CONFIGS[facility_type as usize];

    require!(
        ctx.accounts.player_token_account.amount >= cost,
        WeedMinerError::InsufficientBits
    );

    let burn_amount = cost * gs.burn_rate as u64 / 100;
    let fees_amount = cost - burn_amount;

    // === EFFECTS ===
    // Update player facility and state
    player.facility.facility_type = facility_type;
    player.facility.total_machines = total_machines;
    player.facility.power_output = power_output;
    player.last_upgrade_slot = slot;
    player.last_acc_bits_per_hash = gs.acc_bits_per_hash;

    // Update global state for burned tokens
    gs.burned_tokens = gs.burned_tokens.saturating_add(burn_amount);

    // === INTERACTIONS ===
    // Burn BITS
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.token_mint.to_account_info(),
                from: ctx.accounts.player_token_account.to_account_info(),
                authority: ctx.accounts.player_wallet.to_account_info(),
            },
        ),
        burn_amount,
    )?;
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.player_token_account.to_account_info(),
                to: ctx.accounts.fees_token_account.to_account_info(),
                authority: ctx.accounts.player_wallet.to_account_info(),
            },
        ),
        fees_amount,
    )?;

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  CLAIM REWARDS
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ WeedMinerError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        constraint = player_token_account.owner == player_wallet.key(),
        constraint = player_token_account.mint == global_state.token_mint
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = player.referrer.is_some() && referrer_token_account.owner == player.referrer.unwrap() @ WeedMinerError::InvalidReferrer,
        constraint = referrer_token_account.mint == global_state.token_mint @ WeedMinerError::InvalidTokenMint
    )]
    pub referrer_token_account: Option<Box<Account<'info, TokenAccount>>>,
    #[account(
        mut,
        constraint = fees_token_account.mint == global_state.token_mint,
        constraint = fees_token_account.owner == global_state.fees_wallet @ WeedMinerError::Unauthorized
    )]
    pub fees_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
    let now = Clock::get()?.slot;

    settle_and_mint_rewards(
        &mut ctx.accounts.player,
        &mut ctx.accounts.global_state,
        now,
        &ctx.accounts.player_token_account.to_account_info(),
        ctx.accounts
            .referrer_token_account
            .as_ref()
            .map(|a| a.to_account_info())
            .as_ref(),
        &ctx.accounts.fees_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct ToggleProduction<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority @ WeedMinerError::Unauthorized
    )]
    pub global_state: Account<'info, GlobalState>,
}

pub fn toggle_production(ctx: Context<ToggleProduction>, enable: bool) -> Result<()> {
    let global_state = &mut ctx.accounts.global_state;
    global_state.production_enabled = enable;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateParameters<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority @ WeedMinerError::Unauthorized
    )]
    pub global_state: Account<'info, GlobalState>,
}

pub fn update_parameters(
    ctx: Context<UpdateParameters>,
    referral_fee: Option<u8>,
    burn_rate: Option<u8>,
    cooldown_slots: Option<u64>,
    halving_interval: Option<u64>,
    dust_threshold_divisor: Option<u64>,
) -> Result<()> {
    let global_state = &mut ctx.accounts.global_state;

    if let Some(fee) = referral_fee {
        require!(fee <= 50, WeedMinerError::InvalidReferralFee); // Max 5.0%
        global_state.referral_fee = fee;
    }

    if let Some(rate) = burn_rate {
        require!(rate <= 100, WeedMinerError::InvalidBurnRate); // Max 100%
        global_state.burn_rate = rate;
    }

    if let Some(slots) = cooldown_slots {
        require!(slots > 0, WeedMinerError::InvalidCooldownSlots);
        global_state.cooldown_slots = slots;
    }

    if let Some(halving) = halving_interval {
        require!(halving > 0, WeedMinerError::InvalidHalvingInterval);
        global_state.halving_interval = halving;
    }

    if let Some(divisor) = dust_threshold_divisor {
        require!(divisor > 0, WeedMinerError::InvalidDustThresholdDivisor);
        global_state.dust_threshold_divisor = divisor;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdatePool<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority @ WeedMinerError::Unauthorized
    )]
    pub global_state: Account<'info, GlobalState>,
}

pub fn update_pool_manual(ctx: Context<UpdatePool>) -> Result<()> {
    let global_state = &mut ctx.accounts.global_state;
    let slot_now: u64 = Clock::get()?.slot;

    update_pool(global_state, slot_now);

    Ok(())
}

#[derive(Accounts)]
pub struct GenerateGlobalRandomReward<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority @ WeedMinerError::Unauthorized
    )]
    pub global_state: Account<'info, GlobalState>,
}

pub fn generate_global_random_reward(
    ctx: Context<GenerateGlobalRandomReward>,
    amount: u64,
    expiry_slots: u64,
) -> Result<()> {
    let slot = Clock::get()?.slot;
    let gs = &mut ctx.accounts.global_state;

    // Increment the global reward counter
    gs.global_reward_counter = gs.global_reward_counter.saturating_add(1);

    gs.global_random_reward = Some(GlobalRandomReward {
        reward_id: gs.global_reward_counter,
        amount,
        generated_slot: slot,
        expiry_slot: slot + expiry_slots,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimGlobalRandomReward<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ WeedMinerError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        constraint = player_token_account.owner == player_wallet.key(),
        constraint = player_token_account.mint == global_state.token_mint
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

pub fn claim_global_random_reward(ctx: Context<ClaimGlobalRandomReward>) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;
    let token_mint = &ctx.accounts.token_mint.key();

    let reward = gs
        .global_random_reward
        .as_ref()
        .ok_or(WeedMinerError::NoPendingReward)?;
    require!(slot <= reward.expiry_slot, WeedMinerError::RewardExpired);

    // Check if player already claimed this reward (by reward_id)
    require!(
        player.last_claimed_global_reward_id < reward.reward_id,
        WeedMinerError::RewardAlreadyClaimed
    );

    // Capture reward details
    let reward_amount_to_mint = reward.amount;
    let reward_id = reward.reward_id;

    // === EFFECTS ===
    // Update player's last claimed reward ID
    player.last_claimed_global_reward_id = reward_id;

    // === INTERACTIONS ===
    // Mint reward to player
    let seeds = &[
        GLOBAL_STATE_SEED,
        token_mint.as_ref(),
        &[ctx.bumps.global_state],
    ];
    let signer = &[&seeds[..]];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.player_token_account.to_account_info(),
                authority: gs.to_account_info(),
            },
            signer,
        ),
        reward_amount_to_mint,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct ResetPlayer<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority @ WeedMinerError::Unauthorized
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    /// CHECK: This is just a system account
    pub player_wallet: AccountInfo<'info>,
}

pub fn reset_player(ctx: Context<ResetPlayer>) -> Result<()> {
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;
    let slot = Clock::get()?.slot;

    // Update pool to current slot
    update_pool(gs, slot);

    // Store the old hashpower to update global state
    let old_hashpower = player.hashpower;

    // Reset player's hashpower and facility
    player.hashpower = 0;
    player.facility = Facility {
        facility_type: 0,
        total_machines: 2,
        power_output: 15,
    };
    player.machines = vec![]; // Clear all machines

    // Update global hashpower
    gs.total_hashpower = gs.total_hashpower.saturating_sub(old_hashpower);

    // Update player's last claim slot and accumulator
    player.last_claim_slot = slot;
    player.last_acc_bits_per_hash = gs.acc_bits_per_hash;

    Ok(())
}

#[derive(Accounts)]
pub struct GambleCommit<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ WeedMinerError::Unauthorized,
        constraint = !player.has_pending_gamble @ WeedMinerError::AlreadyHasPendingGamble,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        constraint = player_token_account.owner == player_wallet.key(),
        constraint = player_token_account.mint == global_state.token_mint
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,
    /// CHECK: This is the fees recipient wallet from global_state
    #[account(
        mut,
        constraint = fees_wallet.key() == global_state.fees_wallet @ WeedMinerError::Unauthorized
    )]
    pub fees_wallet: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn gamble_commit(
    ctx: Context<GambleCommit>,
    randomness_account: Pubkey,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Check if production is enabled
    require!(gs.production_enabled, WeedMinerError::ProductionDisabled);

    // Check if player has enough tokens
    require!(
        ctx.accounts.player_token_account.amount >= amount,
        WeedMinerError::InsufficientBits
    );

    // Parse randomness data
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();

    if randomness_data.seed_slot != clock.slot - 1 {
        msg!("seed_slot: {}", randomness_data.seed_slot);
        msg!("slot: {}", clock.slot);
        return Err(WeedMinerError::RandomnessAlreadyRevealed.into());
    }

    // Track the player's committed values
    player.commit_slot = randomness_data.seed_slot;
    player.randomness_account = randomness_account;
    player.current_gamble_amount = amount;
    player.has_pending_gamble = true;

    // Optional SOL fee (0.01 SOL)
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.player_wallet.to_account_info(),
                to: ctx.accounts.fees_wallet.to_account_info(),
            },
        ),
        100_000_000, // 0.1 SOL
    )?;

    // Burn the gambling tokens immediately
    gs.burned_tokens = gs.burned_tokens.saturating_add(amount);
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.token_mint.to_account_info(),
                from: ctx.accounts.player_token_account.to_account_info(),
                authority: ctx.accounts.player_wallet.to_account_info(),
            },
        ),
        amount,
    )?;

    // Increment total gambles counters
    player.total_gambles = player.total_gambles.saturating_add(1);
    gs.total_global_gambles = gs.total_global_gambles.saturating_add(1);

    msg!(
        "Gamble committed, randomness requested for amount: {}",
        amount
    );
    Ok(())
}

#[derive(Accounts)]
pub struct GambleSettle<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ WeedMinerError::Unauthorized,
        constraint = player.has_pending_gamble @ WeedMinerError::NoPendingGamble,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        constraint = player_token_account.owner == player_wallet.key(),
        constraint = player_token_account.mint == global_state.token_mint
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

pub fn gamble_settle(ctx: Context<GambleSettle>) -> Result<()> {
    let clock: Clock = Clock::get()?;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Verify that the provided randomness account matches the stored one
    if ctx.accounts.randomness_account_data.key() != player.randomness_account {
        return Err(WeedMinerError::InvalidRandomnessAccount.into());
    }

    // Parse randomness data
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();

    if randomness_data.seed_slot != player.commit_slot {
        return Err(WeedMinerError::RandomnessExpired.into());
    }

    // Get the revealed random value
    let revealed_random_value = randomness_data
        .get_value(&clock)
        .map_err(|_| WeedMinerError::RandomnessNotResolved)?;

    // Use revealed random value for slot machine odds (2.5% chance for 10x = ~75% house edge)
    let randomness_result = revealed_random_value[0] % 100 < 3; // ~3% chance to win

    if randomness_result {
        msg!("GAMBLE_RESULT: WIN!");

        // Player wins 10x their original amount
        let win_amount = player.current_gamble_amount * 10;

        player.total_gamble_wins = player.total_gamble_wins.saturating_add(1);
        gs.total_global_gamble_wins = gs.total_global_gamble_wins.saturating_add(1);

        // Store the token mint key in a variable first
        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[
            GLOBAL_STATE_SEED,
            token_mint_key.as_ref(),
            &[ctx.bumps.global_state],
        ];
        let signer = &[&seeds[..]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.player_token_account.to_account_info(),
                    authority: gs.to_account_info(),
                },
                signer,
            ),
            win_amount,
        )?;
    } else {
        msg!("GAMBLE_RESULT: LOSE!");
    }

    // Reset gambling state
    player.has_pending_gamble = false;
    player.current_gamble_amount = 0;
    player.randomness_account = Pubkey::default();
    player.commit_slot = 0;

    Ok(())
}
