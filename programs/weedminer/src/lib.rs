use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod helpers;
pub mod instructions;
pub mod state;

use errors::WeedMinerError;
use instructions::*;
use std::str::FromStr;

const ADMIN: &str = "8kvqgxQG77pv6RvEou8f2kHSWi3rtx8F7MksXUqNLGmn";

declare_id!("weedp8th7Upo4wq694FKg9jqufy8SRqrN8cTHPZnmWs");

#[program]
pub mod weedminer {
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
    pub fn generate_global_random_reward(
        ctx: Context<GenerateGlobalRandomReward>,
        amount: u64,
        expiry_slots: u64,
    ) -> Result<()> {
        instructions::generate_global_random_reward(ctx, amount, expiry_slots)
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
    pub fn purchase_initial_facility(
        ctx: Context<PurchaseInitialFacility>,
        referrer: Option<Pubkey>,
    ) -> Result<()> {
        instructions::purchase_initial_facility(ctx, referrer)
    }

    pub fn buy_machine(ctx: Context<BuyMachine>, machine_type: u8) -> Result<()> {
        instructions::buy_machine(ctx, machine_type)
    }

    pub fn sell_machine(ctx: Context<SellMachine>, machine_index: u8) -> Result<()> {
        instructions::sell_machine(ctx, machine_index)
    }

    pub fn upgrade_facility(ctx: Context<UpgradeFacility>, facility_type: u8) -> Result<()> {
        instructions::upgrade_facility(ctx, facility_type)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        instructions::claim_rewards(ctx)
    }

    pub fn claim_global_random_reward(ctx: Context<ClaimGlobalRandomReward>) -> Result<()> {
        instructions::claim_global_random_reward(ctx)
    }

    pub fn gamble(ctx: Context<Gamble>, amount: u64) -> Result<()> {
        instructions::gamble(ctx, amount)
    }
}

fn enforce_admin(key: &Pubkey) -> Result<()> {
    #[cfg(not(feature = "test"))]
    require!(
        *key == Pubkey::from_str(ADMIN).unwrap(),
        WeedMinerError::Unauthorized
    );
    Ok(())
}
