elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use common_structs::Nonce;
use config::MAX_PERCENT;

pub const BLOCKS_IN_YEAR: u64 = 31_536_000 / 6; // seconds_in_year / 6_seconds_per_block

#[elrond_wasm::module]
pub trait CustomRewardsModule:
    config::ConfigModule + token_send::TokenSendModule + farm_token::FarmTokenModule
{
    fn calculate_extra_rewards_since_last_allocation(&self) -> BigUint {
        let current_block_nonce = self.blockchain().get_block_nonce();
        let last_reward_nonce = self.last_reward_block_nonce().get();

        if current_block_nonce > last_reward_nonce {
            let extra_rewards =
                self.calculate_per_block_rewards(current_block_nonce, last_reward_nonce);
            self.last_reward_block_nonce().set(&current_block_nonce);

            extra_rewards
        } else {
            BigUint::zero()
        }
    }

    fn generate_aggregated_rewards(&self) {
        let extra_rewards = self.calculate_extra_rewards_since_last_allocation();
        if extra_rewards > 0 {
            self.increase_reward_reserve(&extra_rewards);
            self.update_reward_per_share(&extra_rewards);
        }
    }

    fn calculate_rewards_with_apr_limit(
        &self,
        amount: &BigUint,
        current_reward_per_share: &BigUint,
        initial_reward_per_share: &BigUint,
        last_claim_block: u64,
    ) -> BigUint {
        let unbounded_rewards =
            self.calculate_reward(amount, current_reward_per_share, initial_reward_per_share);
        if unbounded_rewards == 0u32 {
            return unbounded_rewards;
        }

        let farming_token_total_liquidity = self.farming_token_total_liquidity().get();
        let max_apr = self.max_annual_percentage_rewards().get();
        let max_rewards_per_block =
            farming_token_total_liquidity * max_apr / MAX_PERCENT / BLOCKS_IN_YEAR;

        let current_block = self.blockchain().get_block_nonce();
        let block_diff = current_block - last_claim_block;
        let max_rewards = max_rewards_per_block * block_diff;

        core::cmp::min(unbounded_rewards, max_rewards)
    }

    #[payable("*")]
    #[endpoint(topUpRewards)]
    fn top_up_rewards(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_amount] payment_amount: BigUint,
    ) {
        self.require_permissions();

        let reward_token_id = self.reward_token_id().get();
        require!(payment_token == reward_token_id, "Invalid token");

        self.increase_reward_reserve(&payment_amount);
    }

    #[endpoint]
    fn end_produce_rewards(&self) {
        self.require_permissions();

        self.generate_aggregated_rewards();
        self.produce_rewards_enabled().set(&false);
    }

    #[endpoint(setPerBlockRewardAmount)]
    fn set_per_block_rewards(&self, per_block_amount: BigUint) {
        self.require_permissions();
        require!(per_block_amount != 0, "Amount cannot be zero");

        self.generate_aggregated_rewards();
        self.per_block_reward_amount().set(&per_block_amount);
    }

    #[endpoint(setMaxApr)]
    fn set_max_apr(&self, max_apr: BigUint) {
        self.require_permissions();
        require!(max_apr != 0, "Max APR cannot be zero");

        self.max_annual_percentage_rewards().set(&max_apr);
    }

    #[endpoint(setMinUnbondEpochs)]
    fn set_min_unbond_epochs(&self, min_unbond_epochs: u64) {
        self.require_permissions();
        self.min_unbond_epochs().set(&min_unbond_epochs);
    }

    fn calculate_per_block_rewards(
        &self,
        current_block_nonce: Nonce,
        last_reward_block_nonce: Nonce,
    ) -> BigUint {
        if current_block_nonce <= last_reward_block_nonce || !self.produces_per_block_rewards() {
            return BigUint::zero();
        }

        let per_block_reward = self.per_block_reward_amount().get();
        let block_nonce_diff = current_block_nonce - last_reward_block_nonce;

        per_block_reward * block_nonce_diff
    }

    fn increase_reward_reserve(&self, amount: &BigUint) {
        self.reward_reserve().update(|reserve| {
            *reserve += amount;
        });
    }

    fn decrease_reward_reserve(&self, amount: &BigUint) {
        self.reward_reserve().update(|reserve| {
            require!(&*reserve >= amount, "Not enough reserves");
            *reserve -= amount;
        })
    }

    fn update_reward_per_share(&self, reward_increase: &BigUint) {
        let farm_token_supply = self.farm_token_supply().get();
        if farm_token_supply > 0 {
            let increase =
                self.calculate_reward_per_share_increase(reward_increase, &farm_token_supply);
            self.reward_per_share().update(|r| *r += increase);
        }
    }

    fn calculate_reward_per_share_increase(
        &self,
        reward_increase: &BigUint,
        farm_token_supply: &BigUint,
    ) -> BigUint {
        &(reward_increase * &self.division_safety_constant().get()) / farm_token_supply
    }

    fn calculate_reward(
        &self,
        amount: &BigUint,
        current_reward_per_share: &BigUint,
        initial_reward_per_share: &BigUint,
    ) -> BigUint {
        if current_reward_per_share > initial_reward_per_share {
            let reward_per_share_diff = current_reward_per_share - initial_reward_per_share;
            amount * &reward_per_share_diff / self.division_safety_constant().get()
        } else {
            BigUint::zero()
        }
    }

    #[endpoint(startProduceRewards)]
    fn start_produce_rewards(&self) {
        self.require_permissions();
        require!(
            self.per_block_reward_amount().get() != 0,
            "Cannot produce zero reward amount"
        );
        require!(
            !self.produce_rewards_enabled().get(),
            "Producing rewards is already enabled"
        );
        let current_nonce = self.blockchain().get_block_nonce();
        self.produce_rewards_enabled().set(&true);
        self.last_reward_block_nonce().set(&current_nonce);
    }

    #[inline(always)]
    fn produces_per_block_rewards(&self) -> bool {
        self.produce_rewards_enabled().get()
    }

    #[view(getRewardPerShare)]
    #[storage_mapper("reward_per_share")]
    fn reward_per_share(&self) -> SingleValueMapper<BigUint>;

    #[view(getRewardReserve)]
    #[storage_mapper("reward_reserve")]
    fn reward_reserve(&self) -> SingleValueMapper<BigUint>;

    #[view(getAnnualPercentageRewards)]
    #[storage_mapper("annualPercentageRewards")]
    fn max_annual_percentage_rewards(&self) -> SingleValueMapper<BigUint>;

    #[view(getFarmingTokenTotalLiquidity)]
    #[storage_mapper("farmingTokenTotalLiquidity")]
    fn farming_token_total_liquidity(&self) -> SingleValueMapper<BigUint>;

    #[view(getMinUnbondEpochs)]
    #[storage_mapper("minUnbondEpochs")]
    fn min_unbond_epochs(&self) -> SingleValueMapper<u64>;
}
