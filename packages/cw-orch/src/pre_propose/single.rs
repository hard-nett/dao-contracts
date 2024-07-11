use cw_orch::{interface, prelude::*};

use dao_pre_propose_single::contract::{execute, instantiate, query};
use dao_pre_propose_single::contract::{ExecuteMsg, InstantiateMsg, QueryMsg};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, Empty)]
pub struct DaoPreProposeSingle;

impl<Chain> Uploadable for DaoPreProposeSingle<Chain> {
    /// Return the path to the wasm file corresponding to the contract
    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path("dao_pre_propose_single")
            .unwrap()
    }
    /// Returns a CosmWasm contract wrapper
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query))
    }
}