use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer},
};

use crate::{constants::*, errors::PonzimonError, helpers::*, state::*};
use switchboard_on_demand::accounts::RandomnessAccountData;

#[event]
pub struct FarmUpgraded {
    pub player: Pubkey,
    pub new_farm_type: u8,
}

#[event]
pub struct CardStaked {
    pub player: Pubkey,
    pub card_index: u8,
}

#[event]
pub struct CardUnstaked {
    pub player: Pubkey,
    pub card_index: u8,
}

#[event]
pub struct CardDiscarded {
    pub player: Pubkey,
    pub card_index: u8,
}

#[event]
pub struct BoosterOpened {
    pub player: Pubkey,
    // Events have a size limit, so we can't log the full card details.
    // We'll log the card types as a simple array.
    pub card_types: [u8; 5],
}

#[event]
pub struct CardsRecycled {
    pub player: Pubkey,
    pub successful_upgrades: u8, // Number of cards that were successfully upgraded
    pub total_recycled: u8,      // Total number of cards that were recycled
}

/// ────────────────────────────────────────────────────────────────────────────
/// INTERNAL: update the global accumulator
/// ────────────────────────────────────────────────────────────────────────────
fn update_pool(gs: &mut GlobalState, slot_now: u64) {
    // Security: If the current slot is before the designated start slot,
    // no rewards should be processed.
    if slot_now < gs.start_slot {
        gs.last_reward_slot = gs.start_slot;
        return;
    }

    if slot_now <= gs.last_reward_slot || gs.total_hashpower == 0 {
        gs.last_reward_slot = slot_now;
        return;
    }
    let rate_now = gs.reward_rate;

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
        gs.reward_rate = 0;
        gs.last_reward_slot = slot_now;
        return;
    }

    // Apply the random multiplier to the base rate
    let effective_rate_now = (rate_now as u128)
        .saturating_mul(gs.reward_rate_multiplier as u128)
        .saturating_div(REWARD_RATE_MULTIPLIER_SCALE as u128);

    let slots_elapsed = (slot_now - gs.last_reward_slot) as u128;
    let mut reward = slots_elapsed
        .checked_mul(effective_rate_now as u128)
        .unwrap_or(u128::MAX);
    reward = reward.min(remaining_supply as u128); // clamp to cap

    gs.acc_tokens_per_hashpower += reward * ACC_SCALE / gs.total_hashpower as u128;
    gs.cumulative_rewards = gs.cumulative_rewards.saturating_add(reward as u64);

    gs.last_reward_slot = slot_now;
}

fn update_staking_pool(gs: &mut GlobalState, slot_now: u64) {
    // Security: If the current slot is before the designated start slot,
    // no rewards should be processed.
    if slot_now < gs.start_slot {
        gs.last_staking_reward_slot = gs.start_slot;
        return;
    }

    if slot_now <= gs.last_staking_reward_slot {
        gs.last_staking_reward_slot = slot_now;
        return;
    }
    if gs.total_staked_tokens == 0 {
        gs.last_staking_reward_slot = slot_now;
        return;
    }

    let slots_elapsed = (slot_now.saturating_sub(gs.last_staking_reward_slot)) as u128;

    // SOL rewards are now pool-based - no accumulation here
    // The SOL balance will be checked during claims from the sol_rewards_wallet

    // Token rewards continue with emission-based system
    let token_reward = slots_elapsed
        .checked_mul(gs.token_reward_rate as u128)
        .unwrap_or(u128::MAX);
    if token_reward > 0 {
        gs.acc_token_rewards_per_token = gs.acc_token_rewards_per_token.saturating_add(
            token_reward
                .saturating_mul(ACC_SCALE)
                .saturating_div(gs.total_staked_tokens as u128),
        );
    }

    gs.last_staking_reward_slot = slot_now;
}

/// Helper to settle and mint rewards for a player.
/// Returns Ok(amount_claimed) or Ok(0) if nothing to claim.
fn settle_and_mint_rewards<'info>(
    player: &mut Box<Account<'info, Player>>,
    gs: &mut Account<'info, GlobalState>,
    now: u64,
    player_token_account: &AccountInfo<'info>,
    token_mint: &AccountInfo<'info>,
    rewards_vault: &AccountInfo<'info>,
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
    let pending_u128 = (player.total_hashpower as u128)
        .checked_mul(
            gs.acc_tokens_per_hashpower
                .saturating_sub(player.last_acc_tokens_per_hashpower),
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
        player.last_acc_tokens_per_hashpower = gs.acc_tokens_per_hashpower;
        return Ok(0);
    }

    // update player bookkeeping
    player.last_claim_slot = now;
    player.last_acc_tokens_per_hashpower = gs.acc_tokens_per_hashpower;

    // Give the player their full rewards - no deduction for referrals
    let player_amount = pending;

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

    // mint to player - they get their full rewards
    token::transfer(
        CpiContext::new_with_signer(
            token_program.clone(),
            Transfer {
                from: rewards_vault.clone(),
                to: player_token_account.clone(),
                authority: gs.to_account_info(),
            },
            signer,
        ),
        player_amount,
    )?;

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
        + 8  + 16 + 8           /* reward_rate + acc_tokens_per_hashpower + last_reward_slot */
        + 1  + 1 + 1 + 8 + 8    /* burn_rate + referral_fee + prod + cooldown + dust_divisor */
        + 8 + 8 + 8             /* initial_farm_purchase_fee_lamports + booster_pack_cost_microtokens + gamble_fee_lamports */
        + 8 + 8                 /* total_berries + total_hashpower */
        + 8 + 8                 /* total_global_gambles + total_global_gamble_wins */
        + 8 + 8 + 8             /* total_booster_packs_opened + total_card_recycling_attempts + total_successful_card_recycling */
        + 32 + 8 + 8 + 16 + 16 + 8 + 8 + 8 /* staking: sol_rewards_wallet + total_staked_tokens + staking_lockup_slots + acc_sol_rewards_per_token + acc_token_rewards_per_token + last_staking_reward_slot + token_reward_rate + total_sol_deposited */
        + 8 + 8                 /* dynamic rewards: reward_rate_multiplier + last_rate_update_slot */
        + 64, /* padding for future expansion */
        seeds=[GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,
    /// CHECK: This is the fees recipient wallet
    pub fees_wallet: AccountInfo<'info>,
    /// CHECK: This is a PDA for holding SOL rewards
    #[account(
        init,
        payer = authority,
        seeds = [SOL_REWARDS_WALLET_SEED, token_mint.key().as_ref()],
        bump,
        space = 8
    )]
    pub sol_rewards_wallet: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = fees_wallet,
    )]
    pub fees_token_account: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = authority,
        token::mint = token_mint,
        token::authority = global_state,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = token_mint.mint_authority.unwrap() == global_state.key() @ PonzimonError::InvalidMintAuthority
    )]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn initialize_program(
    ctx: Context<InitializeProgram>,
    start_slot: u64,
    total_supply: u64,
    reward_rate: u64,
    cooldown_slots: Option<u64>,
    initial_farm_purchase_fee_lamports: Option<u64>,
    booster_pack_cost_microtokens: Option<u64>,
    gamble_fee_lamports: Option<u64>,
    staking_lockup_slots: u64,
    token_reward_rate: u64,
) -> Result<()> {
    let gs = &mut ctx.accounts.global_state;

    gs.authority = ctx.accounts.authority.key();
    gs.token_mint = ctx.accounts.token_mint.key();
    gs.fees_wallet = ctx.accounts.fees_wallet.key();
    gs.rewards_vault = ctx.accounts.rewards_vault.key();

    gs.total_supply = total_supply;
    gs.burned_tokens = 0;
    gs.cumulative_rewards = 0;

    gs.start_slot = start_slot;
    gs.reward_rate = reward_rate;

    gs.acc_tokens_per_hashpower = 0;
    gs.last_reward_slot = start_slot;

    gs.burn_rate = 75;
    gs.referral_fee = 25;
    gs.production_enabled = true;
    gs.cooldown_slots = cooldown_slots.unwrap_or(108_000); // 12 hours
    gs.dust_threshold_divisor = 1000; // Default to 0.1%

    // Initialize fee configuration with defaults from constants
    gs.initial_farm_purchase_fee_lamports =
        initial_farm_purchase_fee_lamports.unwrap_or(300_000_000); // 0.3 SOL
    gs.booster_pack_cost_microtokens = booster_pack_cost_microtokens.unwrap_or(100_000_000); // 10 tokens
    gs.gamble_fee_lamports = gamble_fee_lamports.unwrap_or(100_000_000); // 0.1 SOL

    gs.total_berries = 0;
    gs.total_hashpower = 0;

    // Initialize new tracking fields
    gs.total_booster_packs_opened = 0;
    gs.total_card_recycling_attempts = 0;
    gs.total_successful_card_recycling = 0;

    // Staking pool
    gs.sol_rewards_wallet = ctx.accounts.sol_rewards_wallet.key();
    gs.total_staked_tokens = 0;
    gs.staking_lockup_slots = staking_lockup_slots;
    gs.acc_sol_rewards_per_token = 0;
    gs.acc_token_rewards_per_token = 0;
    gs.last_staking_reward_slot = start_slot;
    gs.token_reward_rate = token_reward_rate;
    gs.total_sol_deposited = 0;

    // Dynamic rewards
    gs.reward_rate_multiplier = REWARD_RATE_MULTIPLIER_SCALE;
    gs.last_rate_update_slot = start_slot;

    // Mint initial supply to rewards vault
    let preminted_supply = ctx.accounts.token_mint.supply;
    let amount_to_mint = total_supply.saturating_sub(preminted_supply);

    if amount_to_mint > 0 {
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
                    to: ctx.accounts.rewards_vault.to_account_info(),
                    authority: gs.to_account_info(),
                },
                signer,
            ),
            amount_to_mint,
        )?;
    }

    // Set new mint authority to none
    let token_mint_key = ctx.accounts.token_mint.key();
    let seeds = &[
        GLOBAL_STATE_SEED,
        token_mint_key.as_ref(),
        &[ctx.bumps.global_state],
    ];
    let signer = &[&seeds[..]];

    token::set_authority(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::SetAuthority {
                current_authority: gs.to_account_info(),
                account_or_mint: ctx.accounts.token_mint.to_account_info(),
            },
            signer,
        ),
        token::spl_token::instruction::AuthorityType::MintTokens,
        None,
    )?;

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  PURCHASE INITIAL FARM
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct PurchaseInitialFarm<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        init,
        payer = player_wallet,
        space = 8      // discriminator
            + 32       // owner: Pubkey
            + 10       // farm: Farm (1+1+8)
            + (MAX_CARDS_PER_PLAYER as usize * 6) // cards: [Card; MAX_CARDS_PER_PLAYER] - Card = 6 bytes (2+1+2+1)
            + 1        // card_count: u8
            + 16       // staked_cards_bitset: u128 (Changed from 8 to 16)
            + 8        // berries: u64
            + 8        // total_hashpower: u64
            + 33       // referrer: Option<Pubkey> (1+32)
            + 16       // last_acc_tokens_per_hashpower: u128
            + 8        // last_claim_slot: u64
            + 8        // last_upgrade_slot: u64
            + 8        // total_rewards: u64
            + 8        // total_gambles: u64
            + 8        // total_gamble_wins: u64
            // --- Consolidated randomness fields ---
            + 130      // pending_action: PendingRandomAction enum (1 byte disc + 129 for largest Recycle variant)
            + 32       // randomness_account: Pubkey
            + 8        // commit_slot: u64
            // --- Additional player stats ---
            + 8        // total_earnings_for_referrer: u64
            + 8        // total_booster_packs_opened: u64
            + 8        // total_cards_recycled: u64
            + 8        // successful_card_recycling: u64
            + 8        // total_sol_spent: u64
            + 8        // total_tokens_spent: u64
            + 8 + 8 + 16 + 16 + 8  // Staking stats: staked_tokens + last_stake_slot + last_acc_sol_rewards_per_token + last_acc_token_rewards_per_token + claimed_token_rewards
            + 64,      // padding: [u8; 64] for future expansion
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
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
    /// CHECK: This is the referrer's wallet. Optional. If provided, the wallet key is used as the referrer.
    #[account(mut)]
    pub referrer_wallet: Option<AccountInfo<'info>>,
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
    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,
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

pub fn purchase_initial_farm(ctx: Context<PurchaseInitialFarm>) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(gs.production_enabled, PonzimonError::ProductionDisabled);
    require!(
        player.card_count == 0,
        PonzimonError::InitialFarmAlreadyPurchased
    );

    // The referrer is now derived from the optional `referrer_wallet` account.
    let referrer: Option<Pubkey> = ctx.accounts.referrer_wallet.as_ref().map(|acc| acc.key());

    // Prevent self-referral.
    if let Some(ref r) = referrer {
        require!(
            *r != ctx.accounts.player_wallet.key(),
            PonzimonError::SelfReferralNotAllowed
        );
    }

    // Make sure the reward pool is up to date before any state changes.
    update_pool(gs, slot);

    // --- Fee and Referral Logic ---
    if let Some(referrer_wallet) = &ctx.accounts.referrer_wallet {
        // A referrer is provided, so we split the fee into two transfers.
        let referral_fee_lamports = gs
            .initial_farm_purchase_fee_lamports
            .saturating_mul(gs.referral_fee as u64)
            .saturating_div(100);

        let fees_wallet_amount = gs
            .initial_farm_purchase_fee_lamports
            .saturating_sub(referral_fee_lamports);

        // 1. Transfer the referral commission to the referrer's wallet.
        if referral_fee_lamports > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.player_wallet.to_account_info(),
                        to: referrer_wallet.to_account_info(),
                    },
                ),
                referral_fee_lamports,
            )?;
            player.total_earnings_for_referrer = player
                .total_earnings_for_referrer
                .saturating_add(referral_fee_lamports);
        }

        // 2. Transfer the remaining protocol fee to the main fees wallet.
        if fees_wallet_amount > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.player_wallet.to_account_info(),
                        to: ctx.accounts.fees_wallet.to_account_info(),
                    },
                ),
                fees_wallet_amount,
            )?;
        }
    } else {
        // No referrer, so the entire fee goes to the protocol wallet in a single transfer.
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player_wallet.to_account_info(),
                    to: ctx.accounts.fees_wallet.to_account_info(),
                },
            ),
            gs.initial_farm_purchase_fee_lamports,
        )?;
    }

    // player bootstrap
    player.owner = ctx.accounts.player_wallet.key();
    let (total_cards, berry_capacity, _) = FARM_CONFIGS[1];
    player.farm = Farm {
        farm_type: 1,
        total_cards,
        berry_capacity,
    };

    // Initialize arrays
    player.cards = [Card::default(); MAX_CARDS_PER_PLAYER as usize];
    player.card_count = 0;
    player.staked_cards_bitset = 0; // No cards staked initially

    // Give player 3 starter cards using the IDs from data.ts (not staked initially)
    for &card_id in STARTER_CARD_IDS.iter() {
        if let Some((rarity, hashpower, berry_consumption)) = get_card_by_id(card_id) {
            let card = Card {
                id: card_id,
                rarity,
                hashpower,
                berry_consumption,
            };
            player.add_card(card)?;
        }
    }

    player.berries = 0; // No cards staked initially
    player.total_hashpower = 0; // No cards staked initially
    player.referrer = referrer;
    player.last_claim_slot = slot;
    player.last_upgrade_slot = slot;
    player.total_rewards = 0;
    player.last_acc_tokens_per_hashpower = gs.acc_tokens_per_hashpower;

    // Initialize gambling fields
    player.total_gambles = 0;
    player.total_gamble_wins = 0;
    player.pending_action = PendingRandomAction::None;
    // verify randomness account data is valid
    #[cfg(not(feature = "test"))]
    {
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();
    }
    player.randomness_account = ctx.accounts.randomness_account_data.key();
    player.commit_slot = 0;

    // Initialize new tracking fields
    player.total_earnings_for_referrer = 0;
    player.total_booster_packs_opened = 0;
    player.total_cards_recycled = 0;
    player.successful_card_recycling = 0;
    player.total_sol_spent = gs.initial_farm_purchase_fee_lamports;
    player.total_tokens_spent = 0;

    // Initialize staking stats
    player.staked_tokens = 0;
    player.last_stake_slot = 0;
    player.last_acc_sol_rewards_per_token = 0;
    player.last_acc_token_rewards_per_token = 0;
    player.claimed_token_rewards = 0;

    // Initialize padding field
    player.padding = [0u8; 64];

    // global stats (Effect) - no initial berry consumption since cards aren't staked
    // gs.total_berries += 0; // No change needed

    emit!(InitialFarmPurchased {
        player_wallet: ctx.accounts.player_wallet.key(),
        player_account: player.key(),
        referrer,
        farm_type: player.farm.farm_type,
        initial_cards: player.card_count,
        initial_hashpower: player.berries, // 0 since no cards are staked initially
        slot,
    });

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  DISCARD CARD
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(card_index: u8)]
pub struct DiscardCard<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = player_token_account.mint == global_state.token_mint,
        constraint = player_token_account.owner == player_wallet.key() @ PonzimonError::InvalidTokenAccountOwner
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

pub fn discard_card(ctx: Context<DiscardCard>, card_index: u8) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(gs.production_enabled, PonzimonError::ProductionDisabled);

    // Security: Validate card index bounds
    validate_card_index(card_index, player.card_count as usize)?;

    // Ensure the card is not currently staked
    require!(
        !player.is_card_staked(card_index),
        PonzimonError::CardIsStaked
    );

    // Ensure the card is not currently being recycled
    require!(
        !player.is_card_being_recycled(card_index),
        PonzimonError::CardIsStaked // Reusing this error for consistency
    );

    settle_and_mint_rewards(
        player,
        gs,
        slot,
        &ctx.accounts.player_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.rewards_vault.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    // Remove the card using the helper function
    player.remove_card(card_index)?;

    player.last_acc_tokens_per_hashpower = gs.acc_tokens_per_hashpower;

    emit!(CardDiscarded {
        player: player.key(),
        card_index,
    });

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  STAKE CARD
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(card_index: u8)]
pub struct StakeCard<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    #[account(
        constraint = token_mint.key() == global_state.token_mint @ PonzimonError::InvalidTokenMint
    )]
    pub token_mint: Account<'info, Mint>,
    #[account(
        mut,
        constraint = player_token_account.mint == global_state.token_mint,
        constraint = player_token_account.owner == player_wallet.key() @ PonzimonError::InvalidTokenAccountOwner
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

pub fn stake_card(ctx: Context<StakeCard>, card_index: u8) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Settle rewards before making changes
    settle_and_mint_rewards(
        player,
        gs,
        slot,
        &ctx.accounts.player_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.rewards_vault.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    // Security: Validate card index bounds
    validate_card_index(card_index, player.card_count as usize)?;

    require!(
        !player.is_card_staked(card_index),
        PonzimonError::CardIsStaked // Using for "already staked"
    );

    // Ensure the card is not currently being recycled
    require!(
        !player.is_card_being_recycled(card_index),
        PonzimonError::CardIsStaked // Reusing this error for consistency
    );

    require!(
        player.count_staked_cards() < player.farm.total_cards,
        PonzimonError::MachineCapacityExceeded
    );

    let card = &player.cards[card_index as usize];
    let card_berry_consumption = card.berry_consumption as u64;
    let card_hashpower = card.hashpower as u64;

    // Security: Use safe arithmetic for berry and power calculations
    let new_player_berries = safe_add_berries(player.berries, card_berry_consumption)?;
    let new_total_berries = safe_add_berries(gs.total_berries, card_berry_consumption)?;
    let new_player_hashpower = safe_add_hashpower(player.total_hashpower, card_hashpower)?;
    let new_total_hashpower = safe_add_hashpower(gs.total_hashpower, card_hashpower)?;

    require!(
        new_player_berries <= player.farm.berry_capacity,
        PonzimonError::PowerCapacityExceeded
    );

    // Effects
    player.stake_card(card_index)?;
    player.berries = new_player_berries;
    player.total_hashpower = new_player_hashpower;
    gs.total_berries = new_total_berries;
    gs.total_hashpower = new_total_hashpower;

    emit!(CardStaked {
        player: player.key(),
        card_index,
    });

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  UNSTAKE CARD
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(card_index: u8)]
pub struct UnstakeCard<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    #[account(
        constraint = token_mint.key() == global_state.token_mint @ PonzimonError::InvalidTokenMint
    )]
    pub token_mint: Account<'info, Mint>,
    #[account(
        mut,
        constraint = player_token_account.mint == global_state.token_mint,
        constraint = player_token_account.owner == player_wallet.key() @ PonzimonError::InvalidTokenAccountOwner
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

pub fn unstake_card(ctx: Context<UnstakeCard>, card_index: u8) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Settle rewards before making changes
    settle_and_mint_rewards(
        player,
        gs,
        slot,
        &ctx.accounts.player_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.rewards_vault.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    // Security: Validate card index bounds
    validate_card_index(card_index, player.card_count as usize)?;

    require!(
        player.is_card_staked(card_index),
        PonzimonError::CardNotStaked
    );

    // Ensure the card is not currently being recycled
    require!(
        !player.is_card_being_recycled(card_index),
        PonzimonError::CardIsStaked // Reusing this error for consistency
    );

    let card = &player.cards[card_index as usize];
    let card_berry_consumption = card.berry_consumption as u64;
    let card_hashpower = card.hashpower as u64;

    // Security: Use safe arithmetic for berry and power calculations
    let new_player_berries = safe_sub_berries(player.berries, card_berry_consumption)?;
    let new_total_berries = safe_sub_berries(gs.total_berries, card_berry_consumption)?;
    let new_player_hashpower = safe_sub_hashpower(player.total_hashpower, card_hashpower)?;
    let new_total_hashpower = safe_sub_hashpower(gs.total_hashpower, card_hashpower)?;

    // Effects
    player.unstake_card(card_index)?;
    player.berries = new_player_berries;
    player.total_hashpower = new_player_hashpower;
    gs.total_berries = new_total_berries;
    gs.total_hashpower = new_total_hashpower;

    emit!(CardUnstaked {
        player: player.key(),
        card_index,
    });

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  UPGRADE FARM
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(farm_type: u8)]
pub struct UpgradeFarm<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = player.farm.farm_type + 1 == farm_type  && (farm_type as usize) <= FARM_CONFIGS.len() -1 @ PonzimonError::InvalidFarmType,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = player_token_account.mint == global_state.token_mint,
        constraint = player_token_account.owner == player_wallet.key() @ PonzimonError::InvalidTokenAccountOwner
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
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.rewards_vault.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    let (total_cards, berry_capacity, cost) = FARM_CONFIGS[farm_type as usize];

    require!(
        ctx.accounts.player_token_account.amount >= cost,
        PonzimonError::InsufficientTokens
    );

    let burn_amount = cost * gs.burn_rate as u64 / 100;
    let fees_amount = cost - burn_amount;

    // === EFFECTS ===
    // Update player farm and state
    player.farm.farm_type = farm_type;
    player.farm.total_cards = total_cards;
    player.farm.berry_capacity = berry_capacity;
    player.last_upgrade_slot = slot;
    player.last_acc_tokens_per_hashpower = gs.acc_tokens_per_hashpower;

    // Update global state for burned tokens
    gs.burned_tokens = gs.burned_tokens.saturating_add(burn_amount);

    // Update player spending tracking
    player.total_tokens_spent = player.total_tokens_spent.saturating_add(cost);

    // === INTERACTIONS ===
    // Burn tokens
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

    emit!(FarmUpgraded {
        player: ctx.accounts.player_wallet.key(),
        new_farm_type: farm_type,
    });

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
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
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

pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
    let now = Clock::get()?.slot;

    settle_and_mint_rewards(
        &mut ctx.accounts.player,
        &mut ctx.accounts.global_state,
        now,
        &ctx.accounts.player_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.rewards_vault.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    Ok(())
}

/// OPEN BOOSTER PACK (Secure two-step)

#[derive(Accounts)]
pub struct RequestOpenBooster<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = player.pending_action == PendingRandomAction::None @ PonzimonError::BoosterAlreadyPending,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = player_token_account.mint == global_state.token_mint,
        constraint = player_token_account.owner == player_wallet.key() @ PonzimonError::InvalidTokenAccountOwner
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = fees_token_account.mint == global_state.token_mint,
        constraint = fees_token_account.owner == global_state.fees_wallet @ PonzimonError::Unauthorized
    )]
    pub fees_token_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is the referrer's token account. Optional, but required if the player has a referrer.
    #[account(mut)]
    pub referrer_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,
}

pub fn request_open_booster(ctx: Context<RequestOpenBooster>) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Guards
    require!(gs.production_enabled, PonzimonError::ProductionDisabled);
    require!(
        (player.card_count as usize) + 5 <= MAX_CARDS_PER_PLAYER as usize,
        PonzimonError::MachineCapacityExceeded
    );

    // Settle any pending rewards first
    settle_and_mint_rewards(
        player,
        gs,
        slot,
        &ctx.accounts.player_token_account.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.rewards_vault.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        ctx.bumps.global_state,
    )?;

    // --- Token Fee, Burn, and Referral Logic ---
    let booster_cost = gs.booster_pack_cost_microtokens;

    // Verify the randomness account
    if ctx.accounts.randomness_account_data.key() != player.randomness_account {
        return Err(PonzimonError::InvalidRandomnessAccount.into());
    }
    // Validate Switchboard randomness account
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();
    if randomness_data.seed_slot != slot - 1 {
        return Err(PonzimonError::RandomnessAlreadyRevealed.into());
    }

    // Burn/transfer tokens for the pack
    let burn_amount = booster_cost
        .saturating_mul(gs.burn_rate as u64)
        .saturating_div(100);
    let fees_amount = booster_cost.saturating_sub(burn_amount);

    // First, burn the designated amount from the player's account.
    if burn_amount > 0 {
        gs.burned_tokens = gs.burned_tokens.saturating_add(burn_amount);
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
    }

    // Next, handle the fees, splitting them if a referrer exists.
    if let Some(referrer) = player.referrer {
        require!(
            ctx.accounts.referrer_token_account.clone().unwrap().owner == referrer.key(),
            PonzimonError::ReferrerAccountMissing
        );
        let referral_commission = fees_amount
            .saturating_mul(gs.referral_fee as u64)
            .saturating_div(100);
        let protocol_fee = fees_amount.saturating_sub(referral_commission);

        // Transfer commission to the referrer.
        if referral_commission > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.player_token_account.to_account_info(),
                        to: ctx
                            .accounts
                            .referrer_token_account
                            .clone()
                            .unwrap()
                            .to_account_info(),
                        authority: ctx.accounts.player_wallet.to_account_info(),
                    },
                ),
                referral_commission,
            )?;
            player.total_earnings_for_referrer = player
                .total_earnings_for_referrer
                .saturating_add(referral_commission);
        }

        // Transfer the remaining fee to the protocol wallet.
        if protocol_fee > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.player_token_account.to_account_info(),
                        to: ctx.accounts.fees_token_account.to_account_info(),
                        authority: ctx.accounts.player_wallet.to_account_info(),
                    },
                ),
                protocol_fee,
            )?;
        }
    } else {
        // No referrer, so the entire fee amount goes to the protocol.
        if fees_amount > 0 {
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
        }
    }

    // Set player state for settlement
    player.pending_action = PendingRandomAction::Booster;
    player.commit_slot = randomness_data.seed_slot;

    // Update player spending tracking
    player.total_tokens_spent = player.total_tokens_spent.saturating_add(booster_cost);

    Ok(())
}

#[derive(Accounts)]
pub struct SettleOpenBooster<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = player.pending_action == PendingRandomAction::Booster @ PonzimonError::NoBoosterPending,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,
}

pub fn settle_open_booster(ctx: Context<SettleOpenBooster>) -> Result<()> {
    let clock: Clock = Clock::get()?;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Security: Validate minimum delay for randomness
    validate_randomness_delay(player.commit_slot, clock.slot)?;

    // Verify the randomness account
    if ctx.accounts.randomness_account_data.key() != player.randomness_account {
        return Err(PonzimonError::InvalidRandomnessAccount.into());
    }
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();
    if randomness_data.seed_slot != player.commit_slot {
        return Err(PonzimonError::RandomnessExpired.into());
    }
    let random_value = randomness_data
        .get_value(&clock)
        .map_err(|_| PonzimonError::RandomnessNotResolved)?;
    msg!("random_value ---- {:?}", random_value);

    // Settle rewards before changing berry consumption
    update_pool(gs, clock.slot);
    player.last_acc_tokens_per_hashpower = gs.acc_tokens_per_hashpower;

    // --- Check if it's time to update the reward rate multiplier ---
    if clock.slot
        >= gs
            .last_rate_update_slot
            .saturating_add(REWARD_RATE_UPDATE_COOLDOWN_SLOTS)
    {
        // Use some bytes from the random value to determine the new multiplier.
        // Let's use bytes 28-29 for this.
        let mut multiplier_bytes: [u8; 2] = [0; 2];
        multiplier_bytes.copy_from_slice(&random_value[28..30]);
        let random_u16 = u16::from_le_bytes(multiplier_bytes);

        // This creates a range from 500 to 1500, which is 0.5x to 1.5x
        // The average is 1000 (or 1.0x), keeping your economy balanced.
        let new_multiplier = 500 + (random_u16 as u64 % 1001);

        gs.reward_rate_multiplier = new_multiplier;
        gs.last_rate_update_slot = clock.slot;
        msg!(
            "Reward rate multiplier updated to {}/{}",
            new_multiplier,
            REWARD_RATE_MULTIPLIER_SCALE
        );
    }

    let mut card_ids = [0u16; 5];
    for i in 0..5 {
        // Use a different slice of the random value for each card
        let slice_start = i * 4;
        let slice_end = slice_start + 4;
        let mut random_bytes: [u8; 4] = [0; 4];
        random_bytes.copy_from_slice(&random_value[slice_start..slice_end]);
        let random_u32 = u32::from_le_bytes(random_bytes);

        let random_percent = random_u32 % 1000;

        let rarity = match random_percent {
            0..=499 => COMMON,        // 50.0%
            500..=749 => UNCOMMON,    // 25.0%
            750..=899 => RARE,        // 15.0%
            900..=959 => DOUBLE_RARE, // 6.0%
            960..=989 => VERY_RARE,   // 3.0%
            990..=998 => SUPER_RARE,  // 0.9%
            _ => MEGA_RARE,           // 0.1%
        };

        // Find a random card of the determined rarity
        let cards_of_rarity: Vec<&(u16, u8, u16, u8)> = CARD_DATA
            .iter()
            .filter(|(_, card_rarity, _, _)| *card_rarity == rarity)
            .collect();

        if !cards_of_rarity.is_empty() {
            let card_index = (random_u32 as usize) % cards_of_rarity.len();
            let (card_id, _, hashpower, berry_consumption) = cards_of_rarity[card_index];

            require!(
                (player.card_count as usize) < MAX_CARDS_PER_PLAYER as usize,
                PonzimonError::MachineCapacityExceeded
            );

            let new_card = Card {
                id: *card_id,
                rarity,
                hashpower: *hashpower,
                berry_consumption: *berry_consumption,
            };
            player.add_card(new_card)?;
            card_ids[i] = *card_id;
        }
    }

    // Reset booster state
    player.pending_action = PendingRandomAction::None;
    player.commit_slot = 0;

    // Update tracking statistics
    player.total_booster_packs_opened = player.total_booster_packs_opened.saturating_add(1);
    gs.total_booster_packs_opened = gs.total_booster_packs_opened.saturating_add(1);

    emit!(BoosterOpened {
        player: player.key(),
        card_types: card_ids.map(|id| id as u8), // Convert to u8 for compatibility with event size limits
    });

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

/// Updates a single parameter in the global state.
///
/// # Arguments
///
/// * `ctx` - The context for the instruction.
/// * `parameter_index` - The index of the parameter to update:
///     - 0: ReferralFee (u8)
///     - 1: BurnRate (u8)
///     - 2: CooldownSlots (u64)
///     - 3: DustThresholdDivisor (u64)
///     - 4: InitialFarmPurchaseFeeLamports (u64)
///     - 5: BoosterPackCostMicrotokens (u64)
///     - 6: GambleFeeLamports (u64)
///     - 7: StakingLockupSlots (u64)
///     - 8: TokenRewardRate (u64)
///     - 9: RewardRate (u64)
/// * `parameter_value` - The new value for the parameter.
pub fn update_parameter(
    ctx: Context<UpdateParameters>,
    parameter_index: u8,
    parameter_value: u64,
) -> Result<()> {
    let global_state = &mut ctx.accounts.global_state;

    match parameter_index {
        0 => {
            // ReferralFee
            require!(parameter_value <= 50, PonzimonError::InvalidReferralFee);
            global_state.referral_fee = parameter_value as u8;
        }
        1 => {
            // BurnRate
            require!(parameter_value <= 100, PonzimonError::InvalidBurnRate);
            global_state.burn_rate = parameter_value as u8;
        }
        2 => {
            // CooldownSlots
            require!(parameter_value > 0, PonzimonError::InvalidCooldownSlots);
            global_state.cooldown_slots = parameter_value;
        }
        3 => {
            // DustThresholdDivisor
            require!(
                parameter_value > 0,
                PonzimonError::InvalidDustThresholdDivisor
            );
            global_state.dust_threshold_divisor = parameter_value;
        }
        4 => {
            // InitialFarmPurchaseFeeLamports
            global_state.initial_farm_purchase_fee_lamports = parameter_value;
        }
        5 => {
            // BoosterPackCostMicrotokens
            global_state.booster_pack_cost_microtokens = parameter_value;
        }
        6 => {
            // GambleFeeLamports
            global_state.gamble_fee_lamports = parameter_value;
        }
        7 => {
            // StakingLockupSlots
            global_state.staking_lockup_slots = parameter_value;
        }
        8 => {
            // TokenRewardRate
            global_state.token_reward_rate = parameter_value;
        }
        9 => {
            // RewardRate
            global_state.reward_rate = parameter_value;
        }
        _ => return err!(PonzimonError::InvalidParameterIndex),
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
        has_one = authority @ PonzimonError::Unauthorized,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
        constraint = token_mint.key() == global_state.token_mint @ PonzimonError::InvalidTokenMint
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    pub token_mint: Account<'info, Mint>,
    /// CHECK: This is just a system account
    pub player_wallet: AccountInfo<'info>,
}

pub fn reset_player(ctx: Context<ResetPlayer>) -> Result<()> {
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;
    let slot = Clock::get()?.slot;

    // Update pool to current slot
    update_pool(gs, slot);

    // Store the old berry consumption and power to update global state
    let old_berries = player.berries;
    let old_power = player.total_hashpower;

    // Reset player's berry consumption, power, and farm
    player.berries = 0;
    player.total_hashpower = 0;
    let (total_cards, berry_capacity, _) = FARM_CONFIGS[0];
    player.farm = Farm {
        farm_type: 0,
        total_cards,
        berry_capacity,
    };
    player.cards = [Card::default(); MAX_CARDS_PER_PLAYER as usize]; // Clear all cards
    player.card_count = 0;
    player.staked_cards_bitset = 0; // Clear all staked cards

    // Update global berry consumption and power
    gs.total_berries = gs.total_berries.saturating_sub(old_berries);
    gs.total_hashpower = gs.total_hashpower.saturating_sub(old_power);

    // Update player's last claim slot and accumulator
    player.last_claim_slot = slot;
    player.last_acc_tokens_per_hashpower = gs.acc_tokens_per_hashpower;

    // Reset any pending operations
    player.pending_action = PendingRandomAction::None;
    player.randomness_account = Pubkey::default();
    player.commit_slot = 0;

    Ok(())
}

/// RECYCLE CARDS (Secure two-step)

#[derive(Accounts)]
pub struct RecycleCardsCommit<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = player.pending_action == PendingRandomAction::None @ PonzimonError::RecycleAlreadyPending,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,
}

pub fn recycle_cards_commit(ctx: Context<RecycleCardsCommit>, card_indices: Vec<u8>) -> Result<()> {
    let slot = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Guards
    require!(gs.production_enabled, PonzimonError::ProductionDisabled);
    require!(
        !card_indices.is_empty() && card_indices.len() <= 128,
        PonzimonError::InvalidRecycleCardCount
    );
    require!(
        player.card_count as usize >= card_indices.len(),
        PonzimonError::InvalidRecycleCardCount
    );

    // Validate card indices: must be unique, valid, and not staked
    let mut sorted_indices = card_indices.clone();
    sorted_indices.sort();
    for i in 1..sorted_indices.len() {
        require!(
            sorted_indices[i] != sorted_indices[i - 1],
            PonzimonError::DuplicateRecycleCardIndices
        );
    }
    for &index in &card_indices {
        validate_card_index(index, player.card_count as usize)?;
        require!(!player.is_card_staked(index), PonzimonError::CardIsStaked);
    }

    // Verify the randomness account
    if ctx.accounts.randomness_account_data.key() != player.randomness_account {
        return Err(PonzimonError::InvalidRandomnessAccount.into());
    }
    // Validate Switchboard randomness account
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();
    if randomness_data.seed_slot != slot - 1 {
        return Err(PonzimonError::RandomnessAlreadyRevealed.into());
    }

    // Create array from vector (pad with 0s if needed)
    let mut card_indices_array = [0u8; 128];
    for (i, &index) in card_indices.iter().enumerate() {
        card_indices_array[i] = index;
    }

    // Set pending state with card indices
    player.pending_action = PendingRandomAction::Recycle {
        card_indices: card_indices_array,
        card_count: card_indices.len() as u8,
    };
    player.commit_slot = randomness_data.seed_slot;

    // Update recycling attempt tracking
    gs.total_card_recycling_attempts = gs.total_card_recycling_attempts.saturating_add(1);

    Ok(())
}

#[derive(Accounts)]
pub struct RecycleCardsSettle<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = matches!(player.pending_action, PendingRandomAction::Recycle { .. }) @ PonzimonError::NoRecyclePending,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,
}

pub fn recycle_cards_settle(ctx: Context<RecycleCardsSettle>) -> Result<()> {
    let clock: Clock = Clock::get()?;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Security: Validate minimum delay for randomness
    validate_randomness_delay(player.commit_slot, clock.slot)?;

    // Verify the randomness account
    if ctx.accounts.randomness_account_data.key() != player.randomness_account {
        return Err(PonzimonError::InvalidRandomnessAccount.into());
    }
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();
    if randomness_data.seed_slot != player.commit_slot {
        return Err(PonzimonError::RandomnessExpired.into());
    }
    let random_value = randomness_data
        .get_value(&clock)
        .map_err(|_| PonzimonError::RandomnessNotResolved)?;

    // Settle rewards before changing player state
    update_pool(gs, clock.slot);
    player.last_acc_tokens_per_hashpower = gs.acc_tokens_per_hashpower;

    // Extract recycled card data from pending action
    let (card_indices_array, card_count) = if let PendingRandomAction::Recycle {
        card_indices,
        card_count,
    } = player.pending_action
    {
        (card_indices, card_count)
    } else {
        return Err(PonzimonError::NoRecyclePending.into());
    };

    let mut successful_upgrades = 0u8;
    let mut new_cards: Vec<(u16, u8, u16, u8)> = Vec::new(); // Store new cards to add

    // Process each card individually with 20% chance for upgrade
    for i in 0..card_count {
        let card_index = card_indices_array[i as usize];

        // Validate card index is still valid
        if (card_index as usize) >= (player.card_count as usize) {
            continue; // Skip invalid indices
        }

        let card = &player.cards[card_index as usize];
        let current_rarity = card.rarity;

        // Use different slice of random value for each card
        let random_byte_index = (i as usize) % random_value.len();
        let random_percent = (random_value[random_byte_index] as u32) % 100;

        // 20% chance to upgrade to next rarity
        if random_percent < 20 {
            if let Some(next_rarity) = get_next_rarity(current_rarity) {
                // Find a random card of the next rarity
                let cards_of_next_rarity: Vec<&(u16, u8, u16, u8)> = CARD_DATA
                    .iter()
                    .filter(|(_, card_rarity, _, _)| *card_rarity == next_rarity)
                    .collect();

                if !cards_of_next_rarity.is_empty() {
                    // Use additional randomness for card selection
                    let mut random_bytes: [u8; 4] = [0; 4];
                    let start_idx = (i as usize * 4) % (random_value.len() - 3);
                    random_bytes.copy_from_slice(&random_value[start_idx..start_idx + 4]);
                    let random_u32 = u32::from_le_bytes(random_bytes);

                    let card_index_in_rarity = (random_u32 as usize) % cards_of_next_rarity.len();
                    let (card_id, _, hashpower, berry_consumption) =
                        cards_of_next_rarity[card_index_in_rarity];

                    // Store the new card data to add after removing old cards
                    new_cards.push((*card_id, next_rarity, *hashpower, *berry_consumption));
                    successful_upgrades += 1;
                }
            }
        }
        // 80% chance: card is lost (no new card generated)
    }

    // Remove the recycled cards (must sort descending to not mess up indices)
    let mut card_indices_vec: Vec<u8> = card_indices_array[0..card_count as usize].to_vec();
    card_indices_vec.sort_by(|a, b| b.cmp(a));

    for &index in &card_indices_vec {
        if (index as usize) < (player.card_count as usize) {
            player.remove_card(index)?;
        }
    }

    // Add the new upgraded cards
    for (card_id, rarity, hashpower, berry_consumption) in new_cards {
        require!(
            (player.card_count as usize) < MAX_CARDS_PER_PLAYER as usize,
            PonzimonError::MachineCapacityExceeded
        );

        let new_card = Card {
            id: card_id,
            rarity,
            hashpower,
            berry_consumption,
        };
        player.add_card(new_card)?;
    }

    // Reset recycle state
    player.pending_action = PendingRandomAction::None;
    player.commit_slot = 0;

    // Update tracking statistics
    player.total_cards_recycled = player
        .total_cards_recycled
        .saturating_add(card_count as u64);

    if successful_upgrades > 0 {
        player.successful_card_recycling = player
            .successful_card_recycling
            .saturating_add(successful_upgrades as u64);
        gs.total_successful_card_recycling = gs
            .total_successful_card_recycling
            .saturating_add(successful_upgrades as u64);
    }

    emit!(CardsRecycled {
        player: player.key(),
        successful_upgrades,
        total_recycled: card_count,
    });

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  ADMIN: UPDATE SOL REWARDS POOL
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct UpdateSolRewards<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority @ PonzimonError::Unauthorized,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    /// CHECK: This is a PDA for holding SOL rewards
    #[account(
        mut,
        seeds = [SOL_REWARDS_WALLET_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub sol_rewards_wallet: AccountInfo<'info>,
    pub token_mint: Account<'info, Mint>,
}

pub fn update_sol_rewards(ctx: Context<UpdateSolRewards>) -> Result<()> {
    let gs = &mut ctx.accounts.global_state;

    // Get current SOL balance in the rewards wallet
    let current_balance = ctx.accounts.sol_rewards_wallet.lamports();

    // Calculate how much new SOL was deposited
    let new_sol_deposited = current_balance.saturating_sub(
        gs.total_sol_deposited.saturating_sub(
            // Subtract any SOL that was already claimed/distributed
            gs.acc_sol_rewards_per_token
                .saturating_mul(gs.total_staked_tokens as u128)
                .saturating_div(ACC_SCALE) as u64,
        ),
    );

    if new_sol_deposited > 0 && gs.total_staked_tokens > 0 {
        // Update the SOL accumulator - distribute new SOL proportionally
        let new_sol_per_token = (new_sol_deposited as u128)
            .saturating_mul(ACC_SCALE)
            .saturating_div(gs.total_staked_tokens as u128);

        gs.acc_sol_rewards_per_token = gs
            .acc_sol_rewards_per_token
            .saturating_add(new_sol_per_token);

        gs.total_sol_deposited = gs.total_sol_deposited.saturating_add(new_sol_deposited);
    }

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  CANCEL PENDING ACTION
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct CancelPendingAction<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = player.pending_action != PendingRandomAction::None @ PonzimonError::NoPendingAction,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
}

pub fn cancel_pending_action(ctx: Context<CancelPendingAction>) -> Result<()> {
    let player = &mut ctx.accounts.player;
    let clock = Clock::get()?;

    const CANCEL_TIMEOUT_SLOTS: u64 = 100; // Approx. 80 seconds

    require!(
        clock.slot > player.commit_slot + CANCEL_TIMEOUT_SLOTS,
        PonzimonError::CancelTimeoutNotExpired
    );

    // If the action was a gamble or booster pack, the tokens/SOL have already been spent
    // and are not refunded. This is the cost of canceling to prevent abuse.

    // If the action being cancelled was recycling, the submitted cards are forfeited
    // to prevent abuse. The recycling attempt is still counted as a failed one.
    if let PendingRandomAction::Recycle {
        card_indices,
        card_count,
    } = player.pending_action.clone()
    {
        // Must remove cards from highest index to lowest to avoid shifting issues.
        let mut indices_to_remove: Vec<u8> = card_indices[0..card_count as usize].to_vec();
        indices_to_remove.sort_by(|a, b| b.cmp(a)); // Sort descending

        for &index in &indices_to_remove {
            // The card is not staked, so we can just remove it.
            // The remove_card function handles shifting indices correctly.
            if (index as usize) < (player.card_count as usize) {
                player.remove_card(index)?;
            }
        }
    }

    // Reset the player's pending action state, allowing them to try another action.
    player.pending_action = PendingRandomAction::None;
    player.commit_slot = 0;

    Ok(())
}

/// ────────────────────────────────────────────────────────────────────────────
///  STAKING INSTRUCTIONS
/// ────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct StakeTokens<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
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
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is a PDA for holding SOL rewards
    #[account(
        mut,
        seeds = [SOL_REWARDS_WALLET_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub sol_rewards_wallet: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
    let now = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(amount > 0, PonzimonError::ZeroAmount);

    // Settle pending rewards before staking more tokens
    update_staking_pool(gs, now);

    // Calculate and accumulate pending SOL rewards
    let pending_sol = ((player.staked_tokens as u128)
        .checked_mul(
            gs.acc_sol_rewards_per_token
                .saturating_sub(player.last_acc_sol_rewards_per_token),
        )
        .unwrap_or(0)
        / ACC_SCALE) as u64;

    let pending_tokens = ((player.staked_tokens as u128)
        .checked_mul(
            gs.acc_token_rewards_per_token
                .saturating_sub(player.last_acc_token_rewards_per_token),
        )
        .unwrap_or(0)
        / ACC_SCALE) as u64;

    // Transfer pending SOL rewards to user before staking more
    if pending_sol > 0 {
        let token_mint_key = ctx.accounts.token_mint.key();
        let sol_rewards_wallet_seeds = &[
            SOL_REWARDS_WALLET_SEED,
            token_mint_key.as_ref(),
            &[ctx.bumps.sol_rewards_wallet],
        ];
        let sol_rewards_wallet_signer = &[&sol_rewards_wallet_seeds[..]];

        let sol_transfer_ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.sol_rewards_wallet.key(),
            &ctx.accounts.player_wallet.key(),
            pending_sol,
        );

        anchor_lang::solana_program::program::invoke_signed(
            &sol_transfer_ix,
            &[
                ctx.accounts.sol_rewards_wallet.to_account_info(),
                ctx.accounts.player_wallet.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            sol_rewards_wallet_signer,
        )?;
    }

    // Accumulate token rewards to be claimed later (tokens use accumulated claiming)
    player.claimed_token_rewards = player.claimed_token_rewards.saturating_add(pending_tokens);

    // Transfer tokens to the staking vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.player_token_account.to_account_info(),
                to: ctx.accounts.rewards_vault.to_account_info(),
                authority: ctx.accounts.player_wallet.to_account_info(),
            },
        ),
        amount,
    )?;

    // Update state
    gs.total_staked_tokens = gs.total_staked_tokens.saturating_add(amount);
    player.staked_tokens = player.staked_tokens.saturating_add(amount);
    player.last_stake_slot = now;
    player.last_acc_sol_rewards_per_token = gs.acc_sol_rewards_per_token;
    player.last_acc_token_rewards_per_token = gs.acc_token_rewards_per_token;

    Ok(())
}

#[derive(Accounts)]
pub struct UnstakeTokens<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump
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
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is a PDA for holding SOL rewards
    #[account(
        mut,
        seeds = [SOL_REWARDS_WALLET_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub sol_rewards_wallet: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn unstake_tokens(ctx: Context<UnstakeTokens>, amount: u64) -> Result<()> {
    let now = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(amount > 0, PonzimonError::ZeroAmount);
    require!(
        player.staked_tokens >= amount,
        PonzimonError::InsufficientStake
    );
    require!(
        now >= player.last_stake_slot + gs.staking_lockup_slots,
        PonzimonError::StakeLocked
    );

    // Settle pending rewards
    update_staking_pool(gs, now);

    // Calculate pending SOL rewards (but don't accumulate them)
    let pending_sol = ((player.staked_tokens as u128)
        .checked_mul(
            gs.acc_sol_rewards_per_token
                .saturating_sub(player.last_acc_sol_rewards_per_token),
        )
        .unwrap_or(0)
        / ACC_SCALE) as u64;

    let pending_tokens = ((player.staked_tokens as u128)
        .checked_mul(
            gs.acc_token_rewards_per_token
                .saturating_sub(player.last_acc_token_rewards_per_token),
        )
        .unwrap_or(0)
        / ACC_SCALE) as u64;

    player.claimed_token_rewards = player.claimed_token_rewards.saturating_add(pending_tokens);

    // Transfer SOL rewards to user (must claim on unstake!)
    if pending_sol > 0 {
        let token_mint_key = ctx.accounts.token_mint.key();
        let sol_rewards_wallet_seeds = &[
            SOL_REWARDS_WALLET_SEED,
            token_mint_key.as_ref(),
            &[ctx.bumps.sol_rewards_wallet],
        ];
        let sol_rewards_wallet_signer = &[&sol_rewards_wallet_seeds[..]];

        let sol_transfer_ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.sol_rewards_wallet.key(),
            &ctx.accounts.player_wallet.key(),
            pending_sol,
        );

        anchor_lang::solana_program::program::invoke_signed(
            &sol_transfer_ix,
            &[
                ctx.accounts.sol_rewards_wallet.to_account_info(),
                ctx.accounts.player_wallet.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            sol_rewards_wallet_signer,
        )?;
    }

    // Transfer tokens from vault
    let token_mint_key = ctx.accounts.token_mint.key();
    let seeds = &[
        GLOBAL_STATE_SEED,
        token_mint_key.as_ref(),
        &[ctx.bumps.global_state],
    ];
    let signer = &[&seeds[..]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.rewards_vault.to_account_info(),
                to: ctx.accounts.player_token_account.to_account_info(),
                authority: gs.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    // Update state
    gs.total_staked_tokens = gs.total_staked_tokens.saturating_sub(amount);
    player.staked_tokens = player.staked_tokens.saturating_sub(amount);
    player.last_acc_sol_rewards_per_token = gs.acc_sol_rewards_per_token;
    player.last_acc_token_rewards_per_token = gs.acc_token_rewards_per_token;

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimStakingRewards<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = player_token_account.owner == player_wallet.key(),
        constraint = player_token_account.mint == global_state.token_mint
    )]
    pub player_token_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is a PDA for holding SOL rewards
    #[account(
        mut,
        seeds = [SOL_REWARDS_WALLET_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub sol_rewards_wallet: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn claim_staking_rewards(ctx: Context<ClaimStakingRewards>) -> Result<()> {
    let now = Clock::get()?.slot;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    require!(player.staked_tokens > 0, PonzimonError::InsufficientStake);

    update_staking_pool(gs, now);

    // Calculate SOL rewards using accumulator system (similar to token rewards)
    let pending_sol = ((player.staked_tokens as u128)
        .checked_mul(
            gs.acc_sol_rewards_per_token
                .saturating_sub(player.last_acc_sol_rewards_per_token),
        )
        .unwrap_or(0)
        / ACC_SCALE) as u64;

    // Calculate token rewards (emissions-based)
    let pending_tokens = ((player.staked_tokens as u128)
        .checked_mul(
            gs.acc_token_rewards_per_token
                .saturating_sub(player.last_acc_token_rewards_per_token),
        )
        .unwrap_or(0)
        / ACC_SCALE) as u64;

    let tokens_to_claim = player.claimed_token_rewards.saturating_add(pending_tokens);

    let token_mint_key = ctx.accounts.token_mint.key();
    let seeds = &[
        GLOBAL_STATE_SEED,
        token_mint_key.as_ref(),
        &[ctx.bumps.global_state],
    ];
    let signer = &[&seeds[..]];

    // Transfer SOL rewards from pool
    if pending_sol > 0 {
        let sol_rewards_wallet_seeds = &[
            SOL_REWARDS_WALLET_SEED,
            token_mint_key.as_ref(),
            &[ctx.bumps.sol_rewards_wallet],
        ];
        let sol_rewards_wallet_signer = &[&sol_rewards_wallet_seeds[..]];

        let sol_transfer_ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.sol_rewards_wallet.key(),
            &ctx.accounts.player_wallet.key(),
            pending_sol,
        );

        anchor_lang::solana_program::program::invoke_signed(
            &sol_transfer_ix,
            &[
                ctx.accounts.sol_rewards_wallet.to_account_info(),
                ctx.accounts.player_wallet.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            sol_rewards_wallet_signer,
        )?;
    }

    // Mint token rewards (emissions)
    if tokens_to_claim > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.rewards_vault.to_account_info(),
                    to: ctx.accounts.player_token_account.to_account_info(),
                    authority: gs.to_account_info(),
                },
                signer,
            ),
            tokens_to_claim,
        )?;
        player.claimed_token_rewards = 0;
    }

    // Update player's accumulator checkpoints
    player.last_acc_sol_rewards_per_token = gs.acc_sol_rewards_per_token;
    player.last_acc_token_rewards_per_token = gs.acc_token_rewards_per_token;

    Ok(())
}

#[derive(Accounts)]
pub struct GambleCommit<'info> {
    #[account(mut)]
    pub player_wallet: Signer<'info>,
    #[account(
        mut,
        constraint = player.owner == player_wallet.key() @ PonzimonError::Unauthorized,
        constraint = player.pending_action == PendingRandomAction::None @ PonzimonError::AlreadyHasPendingGamble,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
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

pub fn gamble_commit(ctx: Context<GambleCommit>, amount: u64) -> Result<()> {
    let clock = Clock::get()?;
    let player = &mut ctx.accounts.player;
    let gs = &mut ctx.accounts.global_state;

    // Check if production is enabled
    require!(gs.production_enabled, PonzimonError::ProductionDisabled);

    // Check if player has enough tokens
    require!(
        ctx.accounts.player_token_account.amount >= amount,
        PonzimonError::InsufficientTokens
    );

    // Verify the randomness account
    if ctx.accounts.randomness_account_data.key() != player.randomness_account {
        return Err(PonzimonError::InvalidRandomnessAccount.into());
    }
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
    player.pending_action = PendingRandomAction::Gamble { amount };

    // Gamble SOL fee
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.player_wallet.to_account_info(),
                to: ctx.accounts.fees_wallet.to_account_info(),
            },
        ),
        gs.gamble_fee_lamports,
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

    // Update player spending tracking
    player.total_sol_spent = player
        .total_sol_spent
        .saturating_add(gs.gamble_fee_lamports);
    player.total_tokens_spent = player.total_tokens_spent.saturating_add(amount);

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
        constraint = matches!(player.pending_action, PendingRandomAction::Gamble { .. }) @ PonzimonError::NoPendingGamble,
        seeds = [PLAYER_SEED, player_wallet.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub player: Box<Account<'info, Player>>,
    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        mut,
        seeds = [REWARDS_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub rewards_vault: Account<'info, TokenAccount>,
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

    // Security: Validate minimum delay for randomness
    validate_randomness_delay(player.commit_slot, clock.slot)?;

    // Verify that the provided randomness account matches the stored one
    if ctx.accounts.randomness_account_data.key() != player.randomness_account {
        return Err(PonzimonError::InvalidRandomnessAccount.into());
    }
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow()).unwrap();
    if randomness_data.seed_slot != player.commit_slot {
        return Err(PonzimonError::RandomnessExpired.into());
    }
    let revealed_random_value = randomness_data
        .get_value(&clock)
        .map_err(|_| PonzimonError::RandomnessNotResolved)?;

    let gamble_amount = if let PendingRandomAction::Gamble { amount } = player.pending_action {
        amount
    } else {
        // Should be unreachable due to the constraint, but good practice
        return Err(PonzimonError::NoPendingGamble.into());
    };

    // Use revealed random value for slot machine odds (2.5% chance for 10x = ~75% house edge)
    let randomness_result = revealed_random_value[0] % 100 < 3; // ~3% chance to win

    if randomness_result {
        msg!("GAMBLE_RESULT: WIN!");

        // Player wins 10x their original amount
        let win_amount = gamble_amount * 10;

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
    player.pending_action = PendingRandomAction::None;
    player.commit_slot = 0;

    Ok(())
}
