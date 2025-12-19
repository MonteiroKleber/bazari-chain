#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod loyalty {
    use ink::prelude::string::String;
    use ink::prelude::vec::Vec;
    use ink::storage::Mapping;

    /// Estrutura de configuração do programa de fidelidade
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct LoyaltyConfig {
        /// Nome do programa
        pub name: String,
        /// Símbolo dos pontos (ex: "PTS")
        pub symbol: String,
        /// Ratio de conversão BZR -> Pontos (1 BZR = X pontos)
        pub bzr_to_points_ratio: u128,
        /// Ratio de conversão Pontos -> BZR (X pontos = 1 BZR)
        pub points_to_bzr_ratio: u128,
        /// Pontos expiram após N dias (0 = nunca)
        pub expiration_days: u32,
    }

    /// Níveis de fidelidade
    #[derive(Debug, Clone, Copy, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum LoyaltyTier {
        Bronze,
        Silver,
        Gold,
        Platinum,
    }

    /// Eventos
    #[ink(event)]
    pub struct PointsIssued {
        #[ink(topic)]
        to: AccountId,
        amount: u128,
        reason: String,
    }

    #[ink(event)]
    pub struct PointsRedeemed {
        #[ink(topic)]
        from: AccountId,
        amount: u128,
        bzr_value: Balance,
    }

    #[ink(event)]
    pub struct PointsTransferred {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct TierUpgrade {
        #[ink(topic)]
        account: AccountId,
        old_tier: LoyaltyTier,
        new_tier: LoyaltyTier,
    }

    /// Storage do contrato
    #[ink(storage)]
    pub struct Loyalty {
        /// Owner do contrato (comerciante)
        owner: AccountId,
        /// Configuração do programa
        config: LoyaltyConfig,
        /// Saldo de pontos por conta
        balances: Mapping<AccountId, u128>,
        /// Total de pontos acumulados por conta (histórico)
        total_earned: Mapping<AccountId, u128>,
        /// Tier atual de cada conta
        tiers: Mapping<AccountId, LoyaltyTier>,
        /// Total de pontos emitidos
        total_supply: u128,
        /// Operadores autorizados a emitir pontos
        operators: Mapping<AccountId, bool>,
    }

    /// Erros
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        NotOwner,
        NotOperator,
        InsufficientBalance,
        InvalidAmount,
        TransferToSelf,
        Overflow,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    impl Loyalty {
        /// Cria novo programa de fidelidade
        #[ink(constructor)]
        pub fn new(config: LoyaltyConfig) -> Self {
            let caller = Self::env().caller();
            let mut operators = Mapping::new();
            operators.insert(caller, &true);

            Self {
                owner: caller,
                config,
                balances: Mapping::new(),
                total_earned: Mapping::new(),
                tiers: Mapping::new(),
                total_supply: 0,
                operators,
            }
        }

        /// Emite pontos para um cliente
        #[ink(message)]
        pub fn issue_points(
            &mut self,
            to: AccountId,
            amount: u128,
            reason: String,
        ) -> Result<()> {
            let caller = Self::env().caller();

            // Verificar se é operador
            if !self.operators.get(caller).unwrap_or(false) {
                return Err(Error::NotOperator);
            }

            if amount == 0 {
                return Err(Error::InvalidAmount);
            }

            // Atualizar saldo
            let current_balance = self.balances.get(to).unwrap_or(0);
            let new_balance = current_balance
                .checked_add(amount)
                .ok_or(Error::Overflow)?;
            self.balances.insert(to, &new_balance);

            // Atualizar total ganho
            let current_earned = self.total_earned.get(to).unwrap_or(0);
            let new_earned = current_earned
                .checked_add(amount)
                .ok_or(Error::Overflow)?;
            self.total_earned.insert(to, &new_earned);

            // Atualizar total supply
            self.total_supply = self.total_supply
                .checked_add(amount)
                .ok_or(Error::Overflow)?;

            // Verificar upgrade de tier
            self.check_tier_upgrade(to, new_earned);

            // Emitir evento
            Self::env().emit_event(PointsIssued { to, amount, reason });

            Ok(())
        }

        /// Resgata pontos por BZR
        #[ink(message)]
        pub fn redeem_points(&mut self, amount: u128) -> Result<Balance> {
            let caller = Self::env().caller();

            let current_balance = self.balances.get(caller).unwrap_or(0);
            if current_balance < amount {
                return Err(Error::InsufficientBalance);
            }

            // Calcular valor em BZR
            let bzr_value = amount
                .checked_div(self.config.points_to_bzr_ratio)
                .ok_or(Error::InvalidAmount)?;

            // Atualizar saldo
            let new_balance = current_balance - amount;
            self.balances.insert(caller, &new_balance);

            // Reduzir total supply
            self.total_supply -= amount;

            // TODO: Transferir BZR do contrato para o caller

            // Emitir evento
            Self::env().emit_event(PointsRedeemed {
                from: caller,
                amount,
                bzr_value,
            });

            Ok(bzr_value)
        }

        /// Transfere pontos para outro usuário
        #[ink(message)]
        pub fn transfer(&mut self, to: AccountId, amount: u128) -> Result<()> {
            let caller = Self::env().caller();

            if caller == to {
                return Err(Error::TransferToSelf);
            }

            let from_balance = self.balances.get(caller).unwrap_or(0);
            if from_balance < amount {
                return Err(Error::InsufficientBalance);
            }

            // Atualizar saldos
            self.balances.insert(caller, &(from_balance - amount));

            let to_balance = self.balances.get(to).unwrap_or(0);
            self.balances.insert(to, &(to_balance + amount));

            // Emitir evento
            Self::env().emit_event(PointsTransferred {
                from: caller,
                to,
                amount,
            });

            Ok(())
        }

        /// Consulta saldo de pontos
        #[ink(message)]
        pub fn balance_of(&self, account: AccountId) -> u128 {
            self.balances.get(account).unwrap_or(0)
        }

        /// Consulta tier do usuário
        #[ink(message)]
        pub fn tier_of(&self, account: AccountId) -> LoyaltyTier {
            self.tiers.get(account).unwrap_or(LoyaltyTier::Bronze)
        }

        /// Consulta total de pontos ganhos
        #[ink(message)]
        pub fn total_earned_of(&self, account: AccountId) -> u128 {
            self.total_earned.get(account).unwrap_or(0)
        }

        /// Adiciona operador
        #[ink(message)]
        pub fn add_operator(&mut self, operator: AccountId) -> Result<()> {
            if Self::env().caller() != self.owner {
                return Err(Error::NotOwner);
            }
            self.operators.insert(operator, &true);
            Ok(())
        }

        /// Remove operador
        #[ink(message)]
        pub fn remove_operator(&mut self, operator: AccountId) -> Result<()> {
            if Self::env().caller() != self.owner {
                return Err(Error::NotOwner);
            }
            self.operators.insert(operator, &false);
            Ok(())
        }

        /// Consulta configuração
        #[ink(message)]
        pub fn get_config(&self) -> LoyaltyConfig {
            self.config.clone()
        }

        /// Consulta total supply
        #[ink(message)]
        pub fn total_supply(&self) -> u128 {
            self.total_supply
        }

        // --- Funções internas ---

        fn check_tier_upgrade(&mut self, account: AccountId, total_earned: u128) {
            let current_tier = self.tiers.get(account).unwrap_or(LoyaltyTier::Bronze);

            let new_tier = if total_earned >= 100_000 {
                LoyaltyTier::Platinum
            } else if total_earned >= 50_000 {
                LoyaltyTier::Gold
            } else if total_earned >= 10_000 {
                LoyaltyTier::Silver
            } else {
                LoyaltyTier::Bronze
            };

            if new_tier != current_tier {
                self.tiers.insert(account, &new_tier);
                Self::env().emit_event(TierUpgrade {
                    account,
                    old_tier: current_tier,
                    new_tier,
                });
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[ink::test]
        fn new_works() {
            let config = LoyaltyConfig {
                name: String::from("Test Points"),
                symbol: String::from("TP"),
                bzr_to_points_ratio: 100,
                points_to_bzr_ratio: 100,
                expiration_days: 0,
            };
            let loyalty = Loyalty::new(config);
            assert_eq!(loyalty.total_supply(), 0);
        }

        #[ink::test]
        fn issue_points_works() {
            let config = LoyaltyConfig {
                name: String::from("Test"),
                symbol: String::from("TP"),
                bzr_to_points_ratio: 100,
                points_to_bzr_ratio: 100,
                expiration_days: 0,
            };
            let mut loyalty = Loyalty::new(config);
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            loyalty
                .issue_points(accounts.bob, 1000, String::from("purchase"))
                .unwrap();

            assert_eq!(loyalty.balance_of(accounts.bob), 1000);
            assert_eq!(loyalty.total_supply(), 1000);
        }
    }
}
