use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer},
};

use crate::{constants::*, errors::PonzimonError, helpers::*, state::*};
use switchboard_on_demand::accounts::RandomnessAccountData;

/// ────────────────────────────────────────────────────────────────────────────
/// INTERNAL: update the global accumulator
/// ────────────────────────────────────────────────────────────────────────────
fn update_pool(gs: &mut GlobalState, slot_now: u64) {
    if slot_now <= gs.last_reward_slot || gs.total_berries == 0 {
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

    gs.acc_bits_per_hash += reward * ACC_SCALE / gs.total_berries as u128;
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
        PonzimonError::CooldownNotExpired
    );

    // calculate pending
    let pending_u128 = (player.berries as u128)
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
        + 8                     /* total_berries */
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

    gs.total_berries = 0;

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  PURCHASE INITIAL FARM
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(referrer: Option<Pubkey>)]
pub struct PurchaseInitialFarm<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        init,
        payer = player_wallet,
        space = 8      // discriminator
            + 32       // owner
            + 10       // farm
            + 4        // cards vec header
            + (50 * 17)// MAX_CARDS (50) * size_of(Card)
            + 8        // berries
            + 33       // referrer
            + 16       // last_acc_bits_per_hash
            + 8        // last_claim_slot
            + 8        // last_upgrade_slot
            + 8        // total_rewards
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
        constraint = fees_wallet.key() == global_state.fees_wallet @ PonzimonError::Unauthorized
    )]
    pub fees_wallet: AccountInfo<'info>,
    #[account(
        mut,
        constraint = token_mint.key() == global_state.token_mint @ PonzimonError::InvalidTokenMint
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
pub struct InitialFarmPurchased {
    pub player_wallet: Pubkey,
    pub player_account: Pubkey,
    pub referrer: Option<Pubkey>,
    pub farm_type: u8,
    pub initial_cards: u8,
    pub initial_hashpower: u64,
    pub slot: u64,
}

pub fn purchase_initial_farm(
    ctx: Context<PurchaseInitialFarm>,
    referrer: Option<Pubkey>,
) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(gs.production_enabled, PonzimonError::ProductionDisabled);
    require!(
        player.cards.is_empty(),
        PonzimonError::InitialFarmAlreadyPurchased
    );

    // Prevent self-referral
    if let Some(ref r) = referrer {
        require!(
            *r != ctx.accounts.player_wallet.key(),
            PonzimonError::SelfReferralNotAllowed
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
    player.farm = Farm {
        farm_type: 0,       // Starter Hut
        total_cards: 3,     // Can hold 3 cards initially
        berry_capacity: 15, // 15 berry capacity
    };

    // Give player 3 starter cards
    let starter_cards = vec![
        Card {
            card_type: COMMON_CARD,
            card_power: 100,
            berry_consumption: 3,
        },
        Card {
            card_type: COMMON_CARD,
            card_power: 100,
            berry_consumption: 3,
        },
        Card {
            card_type: UNCOMMON_CARD,
            card_power: 250,
            berry_consumption: 5,
        },
    ];

    let total_berry_consumption = starter_cards
        .iter()
        .map(|c| c.berry_consumption)
        .sum::<u64>();
    player.cards = starter_cards;
    player.berries = total_berry_consumption; // Total berry consumption
    player.referrer = referrer;
    player.last_claim_slot = slot;
    player.last_upgrade_slot = slot;
    player.total_rewards = 0;
    player.last_acc_bits_per_hash = gs.acc_bits_per_hash;

    // Initialize gambling fields
    player.total_gambles = 0;
    player.total_gamble_wins = 0;

    // Initialize Switchboard randomness fields
    player.randomness_account = Pubkey::default();
    player.commit_slot = 0;
    player.current_gamble_amount = 0;
    player.has_pending_gamble = false;

    // global stats (Effect)
    gs.total_berries += total_berry_consumption;

    emit!(InitialFarmPurchased {
        player_wallet: ctx.accounts.player_wallet.key(),
        player_account: player.key(),
        referrer,
        farm_type: player.farm.farm_type,
        initial_cards: player.cards.len() as u8,
        initial_hashpower: player.berries,
        slot,
    });

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  OPEN BOOSTER PACK
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct OpenBooster<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
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
        constraint = fees_token_account.owner == global_state.fees_wallet @ PonzimonError::Unauthorized
    )]
    pub fees_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn open_booster(ctx: Context<OpenBooster>) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // guards
    require!(gs.production_enabled, PonzimonError::ProductionDisabled);

    // Check if player has space for 5 more cards
    require!(
        player.cards.len() + 5 <= player.farm.total_cards as usize,
        PonzimonError::MachineCapacityExceeded
    );

    // Check if farm has enough berry capacity for new cards
    // Estimate berry consumption for 5 new cards (using average of 8 berries per card)
    let estimated_new_berry_consumption = 5 * 8; // Conservative estimate
    require!(
        player.berries + estimated_new_berry_consumption <= player.farm.berry_capacity,
        PonzimonError::PowerCapacityExceeded
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

    // Booster pack cost: 100 BITS
    let booster_cost = 100_000_000; // 100 BITS in microBITS
    require!(
        ctx.accounts.player_token_account.amount >= booster_cost,
        PonzimonError::InsufficientBits
    );

    // Calculate amounts for burn/transfer
    let burn_amount = booster_cost * gs.burn_rate as u64 / 100;
    let fees_amount = booster_cost - burn_amount;

    // === EFFECTS ===
    // Update global state for burned tokens
    gs.burned_tokens = gs.burned_tokens.saturating_add(burn_amount);

    // Generate 5 random cards using timestamp-based randomness
    let timestamp = Clock::get()?.unix_timestamp;
    let mut total_new_berry_consumption = 0u64;

    for i in 0..5 {
        // Simple pseudo-random number generation using timestamp + card index
        let seed = (timestamp as u64)
            .wrapping_add(i as u64)
            .wrapping_add(player.cards.len() as u64);
        let random_value = seed.wrapping_mul(1103515245).wrapping_add(12345) % 100;

        // Determine card rarity based on probability
        let card_type = match random_value {
            0..=49 => COMMON_CARD,      // 50% chance
            50..=74 => UNCOMMON_CARD,   // 25% chance
            75..=89 => RARE_CARD,       // 15% chance
            90..=95 => HOLO_RARE_CARD,  // 6% chance
            96..=98 => ULTRA_RARE_CARD, // 3% chance
            99 => SECRET_RARE_CARD,     // 1% chance
            _ => COMMON_CARD,
        };

        let (card_power, berry_consumption, _) = CARD_CONFIGS[card_type as usize];

        let new_card = Card {
            card_type,
            card_power,
            berry_consumption,
        };

        total_new_berry_consumption += berry_consumption;
        player.cards.push(new_card);
    }

    // Update player and global berry consumption
    player.berries += total_new_berry_consumption;
    gs.total_berries += total_new_berry_consumption;

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
///  SELL CARD
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(card_index: u8)]
pub struct SellCard<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
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
        constraint = fees_token_account.owner == global_state.fees_wallet @ PonzimonError::Unauthorized
    )]
    pub fees_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

pub fn sell_card(ctx: Context<SellCard>, card_index: u8) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(gs.production_enabled, PonzimonError::ProductionDisabled);

    require!(
        card_index < player.cards.len() as u8,
        PonzimonError::InvalidMachineType
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

    let card = player.cards.remove(card_index as usize);

    player.berries = player.berries.saturating_sub(card.berry_consumption);
    gs.total_berries = gs.total_berries.saturating_sub(card.berry_consumption);

    player.last_acc_bits_per_hash = gs.acc_bits_per_hash;

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  UPGRADE FARM  (hp does not change → only update pool/debt if you add hp)
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(farm_type: u8)]
pub struct UpgradeFarm<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = farm_type > player.farm.farm_type @ PonzimonError::InvalidFarmType,
        constraint = farm_type <= MASTER_ARENA @ PonzimonError::InvalidFarmType,
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
        constraint = fees_token_account.owner == global_state.fees_wallet @ PonzimonError::Unauthorized
    )]
    pub fees_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

pub fn upgrade_farm(ctx: Context<UpgradeFarm>, farm_type: u8) -> Result<()> {
    require!(
        (COZY_CABIN..=MASTER_ARENA).contains(&farm_type),
        PonzimonError::InvalidFarmType
    );

    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    update_pool(gs, slot);

    require!(gs.production_enabled, PonzimonError::ProductionDisabled);
    require!(
        slot >= player.last_upgrade_slot + gs.cooldown_slots,
        PonzimonError::CooldownNotExpired
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

    let (total_cards, berry_capacity, cost) = FARM_CONFIGS[farm_type as usize];

    require!(
        ctx.accounts.player_token_account.amount >= cost,
        PonzimonError::InsufficientBits
    );

    let burn_amount = cost * gs.burn_rate as u64 / 100;
    let fees_amount = cost - burn_amount;

    // === EFFECTS ===
    // Update player farm and state
    player.farm.farm_type = farm_type;
    player.farm.total_cards = total_cards;
    player.farm.berry_capacity = berry_capacity;
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
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
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
        constraint = player.referrer.is_some() && referrer_token_account.owner == player.referrer.unwrap() @ PonzimonError::InvalidReferrer,
        constraint = referrer_token_account.mint == global_state.token_mint @ PonzimonError::InvalidTokenMint
    )]
    pub referrer_token_account: Option<Box<Account<'info, TokenAccount>>>,
    #[account(
        mut,
        constraint = fees_token_account.mint == global_state.token_mint,
        constraint = fees_token_account.owner == global_state.fees_wallet @ PonzimonError::Unauthorized
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
        has_one = authority @ PonzimonError::Unauthorized
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
        has_one = authority @ PonzimonError::Unauthorized
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
        require!(fee <= 50, PonzimonError::InvalidReferralFee); // Max 5.0%
        global_state.referral_fee = fee;
    }

    if let Some(rate) = burn_rate {
        require!(rate <= 100, PonzimonError::InvalidBurnRate); // Max 100%
        global_state.burn_rate = rate;
    }

    if let Some(slots) = cooldown_slots {
        require!(slots > 0, PonzimonError::InvalidCooldownSlots);
        global_state.cooldown_slots = slots;
    }

    if let Some(halving) = halving_interval {
        require!(halving > 0, PonzimonError::InvalidHalvingInterval);
        global_state.halving_interval = halving;
    }

    if let Some(divisor) = dust_threshold_divisor {
        require!(divisor > 0, PonzimonError::InvalidDustThresholdDivisor);
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
        has_one = authority @ PonzimonError::Unauthorized
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
pub struct ResetPlayer<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority @ PonzimonError::Unauthorized
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

    // Store the old berry consumption to update global state
    let old_berries = player.berries;

    // Reset player's berry consumption and farm
    player.berries = 0;
    player.farm = Farm {
        farm_type: 0,
        total_cards: 3,
        berry_capacity: 15,
    };
    player.cards = vec![]; // Clear all cards

    // Update global berry consumption
    gs.total_berries = gs.total_berries.saturating_sub(old_berries);

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
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = !player.has_pending_gamble @ PonzimonError::AlreadyHasPendingGamble,
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
        constraint = fees_wallet.key() == global_state.fees_wallet @ PonzimonError::Unauthorized
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
    require!(gs.production_enabled, PonzimonError::ProductionDisabled);

    // Check if player has enough tokens
    require!(
        ctx.accounts.player_token_account.amount >= amount,
        PonzimonError::InsufficientBits
    );

    // Parse randomness data
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();

    if randomness_data.seed_slot != clock.slot - 1 {
        msg!("seed_slot: {}", randomness_data.seed_slot);
        msg!("slot: {}", clock.slot);
        return Err(PonzimonError::RandomnessAlreadyRevealed.into());
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
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = player.has_pending_gamble @ PonzimonError::NoPendingGamble,
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
        return Err(PonzimonError::InvalidRandomnessAccount.into());
    }

    // Parse randomness data
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();

    if randomness_data.seed_slot != player.commit_slot {
        return Err(PonzimonError::RandomnessExpired.into());
    }

    // Get the revealed random value
    let revealed_random_value = randomness_data
        .get_value(&clock)
        .map_err(|_| PonzimonError::RandomnessNotResolved)?;

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
