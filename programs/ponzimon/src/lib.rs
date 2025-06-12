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

declare_id!("ponHUhwQLrCnHdYiYJcboxwKsA3GxZctCKYJr5SnTwr");

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
    ) -> Result<()> {
        instructions::initialize_program(
            ctx,
            start_slot,
            halving_interval,
            total_supply,
            initial_reward_rate,
            cooldown_slots,
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
    pub fn update_parameters(
        ctx: Context<UpdateParameters>,
        referral_fee: Option<u8>,
        burn_rate: Option<u8>,
        cooldown_slots: Option<u64>,
        halving_interval: Option<u64>,
        dust_threshold_divisor: Option<u64>,
    ) -> Result<()> {
        instructions::update_parameters(
            ctx,
            referral_fee,
            burn_rate,
            cooldown_slots,
            halving_interval,
            dust_threshold_divisor,
        )
    }

    // ────────────────────────────────────────────────────────────────────────────
    ///  NON ADMIN FUNCTIONS
    // ────────────────────────────────────────────────────────────────────────────
    pub fn purchase_initial_farm(
        ctx: Context<PurchaseInitialFarm>,
        referrer: Option<Pubkey>,
    ) -> Result<()> {
        instructions::purchase_initial_farm(ctx, referrer)
    }

    pub fn open_booster(ctx: Context<OpenBooster>) -> Result<()> {
        instructions::open_booster(ctx)
    }

    pub fn sell_card(ctx: Context<SellCard>, card_index: u8) -> Result<()> {
        instructions::sell_card(ctx, card_index)
    }

    pub fn upgrade_farm(ctx: Context<UpgradeFarm>, farm_type: u8) -> Result<()> {
        instructions::upgrade_farm(ctx, farm_type)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        instructions::claim_rewards(ctx)
    }

    pub fn gamble_commit(
        ctx: Context<GambleCommit>,
        randomness_account: Pubkey,
        amount: u64,
    ) -> Result<()> {
        instructions::gamble_commit(ctx, randomness_account, amount)
    }

    pub fn gamble_settle(ctx: Context<GambleSettle>) -> Result<()> {
        instructions::gamble_settle(ctx)
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
