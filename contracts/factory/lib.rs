#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod factory {
    use ink::prelude::vec::Vec;
    use ink::storage::Mapping;

    /// Tipo de contrato
    #[derive(Debug, Clone, Copy, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ContractType {
        Loyalty,
        Escrow,
        RevenueSplit,
    }

    /// Registro de contrato deployado
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct DeployedContract {
        pub contract_type: ContractType,
        pub address: AccountId,
        pub owner: AccountId,
        pub deployed_at: Timestamp,
    }

    /// Eventos
    #[ink(event)]
    pub struct ContractDeployed {
        #[ink(topic)]
        contract_type: ContractType,
        #[ink(topic)]
        owner: AccountId,
        address: AccountId,
    }

    /// Storage
    #[ink(storage)]
    pub struct Factory {
        /// Contador de deploys
        deploy_count: u64,
        /// Contratos por owner
        contracts_by_owner: Mapping<AccountId, Vec<DeployedContract>>,
        /// Code hashes dos templates
        loyalty_code_hash: Hash,
        escrow_code_hash: Hash,
        revenue_split_code_hash: Hash,
    }

    impl Factory {
        #[ink(constructor)]
        pub fn new(
            loyalty_code_hash: Hash,
            escrow_code_hash: Hash,
            revenue_split_code_hash: Hash,
        ) -> Self {
            Self {
                deploy_count: 0,
                contracts_by_owner: Mapping::new(),
                loyalty_code_hash,
                escrow_code_hash,
                revenue_split_code_hash,
            }
        }

        /// Deploy de contrato de fidelidade
        #[ink(message, payable)]
        pub fn deploy_loyalty(
            &mut self,
            name: ink::prelude::string::String,
            symbol: ink::prelude::string::String,
            bzr_to_points_ratio: u128,
            points_to_bzr_ratio: u128,
        ) -> Result<AccountId, ink::prelude::string::String> {
            // TODO: Usar ink::env::instantiate_contract
            // Por enquanto, retornar placeholder
            let caller = Self::env().caller();

            Self::env().emit_event(ContractDeployed {
                contract_type: ContractType::Loyalty,
                owner: caller,
                address: caller, // placeholder
            });

            self.deploy_count += 1;

            Ok(caller)
        }

        /// Consulta contratos de um owner
        #[ink(message)]
        pub fn get_contracts(&self, owner: AccountId) -> Vec<DeployedContract> {
            self.contracts_by_owner.get(owner).unwrap_or_default()
        }

        /// Consulta total de deploys
        #[ink(message)]
        pub fn get_deploy_count(&self) -> u64 {
            self.deploy_count
        }
    }
}
