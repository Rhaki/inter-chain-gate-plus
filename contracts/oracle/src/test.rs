use std::str::FromStr;

use cosmwasm_std::{
    from_binary,
    testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier},
    to_binary, Addr, Decimal, Env, MemoryStorage, MessageInfo, OwnedDeps, QueryRequest, WasmQuery,
};
use gate_pkg::{GateMsg, GateQueryResponse};

use crate::{
    contract::{execute, instantiate, query},
    msgs::{ExecuteMsg, InstantiateMsg, QueryMsg},
};

fn register_and_feed(
    deps: &mut OwnedDeps<MemoryStorage, MockApi, MockQuerier>,
    env: Env,
    info_owner: MessageInfo,
    info_feeder: MessageInfo,
    asset: String,
    price: Decimal,
) {
    // REGISTER AN ASSET

    let msg = ExecuteMsg::RegisterAsset {
        asset: asset.to_string(),
        feeder: info_feeder.sender.clone(),
    };

    let _res = execute(deps.as_mut(), env.clone(), info_owner, msg).unwrap();

    // FEED PRICE

    let msg = ExecuteMsg::FeedPrice { asset, price };

    let _res = execute(deps.as_mut(), env, info_feeder, msg).unwrap();
}

#[test]
fn main() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let owner_info: MessageInfo = mock_info("onwer000", &[]);
    let feeder_info: MessageInfo = mock_info("feeder000", &[]);

    let asset_name = "asset000".to_string();

    let gate_info: MessageInfo = mock_info("gate_contract", &[]);

    let remote_oracle_contract: Addr = Addr::unchecked("remote_oracle_contract");

    // INIT CONTRACT

    let msg = InstantiateMsg {};

    let _res = instantiate(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();

    // REGISTER GATE

    let msg = ExecuteMsg::RegisterGate {
        contract: gate_info.sender.clone(),
    };

    let _res = execute(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();

    // REGISTER ASSET

    register_and_feed(
        &mut deps,
        env.clone(),
        owner_info.clone(),
        feeder_info.clone(),
        asset_name.clone(),
        Decimal::from_str("1").unwrap(),
    );

    // SIMULATE QUERY REQUEST

    let query_r: QueryMsg = QueryMsg::Price {
        asset: asset_name.clone(),
    };

    let res = query(deps.as_ref(), env.clone(), query_r.clone()).unwrap();

    assert_eq!(
        Decimal::from_str("1").unwrap(),
        from_binary::<Decimal>(&res).unwrap()
    );

    let msg = ExecuteMsg::FeedPrice {
        asset: asset_name.clone(),
        price: Decimal::from_str("2").unwrap(),
    };

    let _res = execute(deps.as_mut(), env.clone(), feeder_info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReceiveGateMsg(GateMsg::QueryResponse {
        queries: vec![GateQueryResponse {
            request: QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: remote_oracle_contract.to_string(),
                msg: to_binary(&query_r).unwrap(),
            }),
            response: res,
        }],
        callback_msg: None,
    });

    let _res = execute(deps.as_mut(), env.clone(), gate_info, msg).unwrap();

    let query_r: QueryMsg = QueryMsg::Price { asset: asset_name };

    let res = query(deps.as_ref(), env.clone(), query_r).unwrap();

    assert_eq!(
        Decimal::from_str("1").unwrap(),
        from_binary::<Decimal>(&res).unwrap()
    );

    // QUERY PRICES

    register_and_feed(
        &mut deps,
        env.clone(),
        owner_info.clone(),
        feeder_info.clone(),
        "asset_2".to_string(),
        Decimal::from_str("2").unwrap(),
    );
    register_and_feed(
        &mut deps,
        env.clone(),
        owner_info.clone(),
        feeder_info.clone(),
        "asset_3".to_string(),
        Decimal::from_str("3").unwrap(),
    );
    register_and_feed(
        &mut deps,
        env.clone(),
        owner_info.clone(),
        feeder_info.clone(),
        "asset_4".to_string(),
        Decimal::from_str("4").unwrap(),
    );
    register_and_feed(
        &mut deps,
        env.clone(),
        owner_info,
        feeder_info,
        "asset_5".to_string(),
        Decimal::from_str("5").unwrap(),
    );

    let msg = QueryMsg::Prices {
        start_after: None,
        limit: Some(3),
    };

    let res = query(deps.as_ref(), env.clone(), msg).unwrap();

    let response: Vec<(String, Decimal)> = from_binary(&res).unwrap();

    assert_eq!(3, response.len());

    let msg = QueryMsg::Prices {
        start_after: Some(response.last().unwrap().0.clone()),
        limit: Some(3),
    };

    let res = query(deps.as_ref(), env, msg).unwrap();

    let response: Vec<(String, Decimal)> = from_binary(&res).unwrap();

    assert_eq!(2, response.len());
    assert_eq!(Decimal::from_str("5").unwrap(), response.last().unwrap().1);
}
