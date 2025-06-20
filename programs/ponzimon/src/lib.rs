use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod helpers;
pub mod instructions;
pub mod state;

use errors::PonzimonError;
use instructions::*;
use std::str::FromStr;

const ADMIN: &str = "8kvqgxQG77pv6RvEou8f2kHSWi3rtx8F7MksXUqNLGmn";

declare_id!("PoZ2MCf1TeEep82Wi1TARMg6hoG8x4bNxxU6WGWeznt");

#[program]
pub mod ponzimon {
    use super::*;

    #[access_control(enforce_admin(ctx.accounts.authority.key))]
    pub fn initialize_program(
        ctx: Context<InitializeProgram>,
        start_slot: u64,
        halving_interval: u64,
        total_supply: u64,
        initial_reward_rate: u64,
        cooldown_slots: Option<u64>,
        initial_farm_purchase_fee_lamports: Option<u64>,
        booster_pack_cost_microtokens: Option<u64>,
        gamble_fee_lamports: Option<u64>,
        staking_lockup_slots: u64,
        token_reward_rate: u64,
    ) -> Result<()> {
        instructions::initialize_program(
            ctx,
            start_slot,
            halving_interval,
            total_supply,
            initial_reward_rate,
            cooldown_slots,
            initial_farm_purchase_fee_lamports,
            booster_pack_cost_microtokens,
            gamble_fee_lamports,
            staking_lockup_slots,
            token_reward_rate,
        )
    }
    /// ────────────────────────────────────────────────────────────────────────────
    ///  ALL ADMIN FUNCTIONS ENFORCED BY AUTHORITY SIGNING IXS
    /// ────────────────────────────────────────────────────────────────────────────
    pub fn reset_player(ctx: Context<ResetPlayer>) -> Result<()> {
        instructions::reset_player(ctx)
    }
    pub fn toggle_production(ctx: Context<ToggleProduction>, enable: bool) -> Result<()> {
        instructions::toggle_production(ctx, enable)
    }
    pub fn update_pool_manual(ctx: Context<UpdatePool>) -> Result<()> {
        instructions::update_pool_manual(ctx)
    }
    pub fn update_sol_rewards(ctx: Context<UpdateSolRewards>) -> Result<()> {
        instructions::update_sol_rewards(ctx)
    }
    pub fn update_parameter(
        ctx: Context<UpdateParameters>,
        parameter_index: u8,
        parameter_value: u64,
    ) -> Result<()> {
        instructions::update_parameter(ctx, parameter_index, parameter_value)
    }

    // ────────────────────────────────────────────────────────────────────────────
    ///  NON ADMIN FUNCTIONS
    // ────────────────────────────────────────────────────────────────────────────
    pub fn purchase_initial_farm(ctx: Context<PurchaseInitialFarm>) -> Result<()> {
        instructions::purchase_initial_farm(ctx)
    }

    pub fn stake_card(ctx: Context<StakeCard>, card_index: u8) -> Result<()> {
        instructions::stake_card(ctx, card_index)
    }

    pub fn unstake_card(ctx: Context<UnstakeCard>, card_index: u8) -> Result<()> {
        instructions::unstake_card(ctx, card_index)
    }

    pub fn discard_card(ctx: Context<DiscardCard>, card_index: u8) -> Result<()> {
        instructions::discard_card(ctx, card_index)
    }

    pub fn request_open_booster(ctx: Context<RequestOpenBooster>) -> Result<()> {
        instructions::request_open_booster(ctx)
    }
    pub fn settle_open_booster(ctx: Context<SettleOpenBooster>) -> Result<()> {
        instructions::settle_open_booster(ctx)
    }

    pub fn upgrade_farm(ctx: Context<UpgradeFarm>, farm_type: u8) -> Result<()> {
        instructions::upgrade_farm(ctx, farm_type)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        instructions::claim_rewards(ctx)
    }

    pub fn gamble_commit(ctx: Context<GambleCommit>, amount: u64) -> Result<()> {
        instructions::gamble_commit(ctx, amount)
    }

    pub fn gamble_settle(ctx: Context<GambleSettle>) -> Result<()> {
        instructions::gamble_settle(ctx)
    }

    pub fn recycle_cards_commit(
        ctx: Context<RecycleCardsCommit>,
        card_indices: Vec<u8>,
    ) -> Result<()> {
        instructions::recycle_cards_commit(ctx, card_indices)
    }

    pub fn recycle_cards_settle(ctx: Context<RecycleCardsSettle>) -> Result<()> {
        instructions::recycle_cards_settle(ctx)
    }

    pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
        instructions::stake_tokens(ctx, amount)
    }

    pub fn unstake_tokens(ctx: Context<UnstakeTokens>, amount: u64) -> Result<()> {
        instructions::unstake_tokens(ctx, amount)
    }

    pub fn claim_staking_rewards(ctx: Context<ClaimStakingRewards>) -> Result<()> {
        instructions::claim_staking_rewards(ctx)
    }

    pub fn cancel_pending_action(ctx: Context<CancelPendingAction>) -> Result<()> {
        instructions::cancel_pending_action(ctx)
    }
}

fn enforce_admin(key: &Pubkey) -> Result<()> {
    #[cfg(not(feature = "test"))]
    require!(
        *key == Pubkey::from_str(ADMIN).unwrap(),
        PonzimonError::Unauthorized
    );
    Ok(())
}
