use std::collections::HashMap;

use cosmwasm_std::{
    from_binary,
    testing::{mock_dependencies, mock_env, mock_info},
    to_binary, Addr, Coin, Uint128,
};

use gate_pkg::GateMsg;

use crate::{
    contract::{execute, instantiate, query},
    msgs::{
        self, BridgeMsgInfo, ExecuteMsg, GateCollectMsgsAllowed, InstantiateMsg, Position, QueryMsg,
    },
};

#[test]
fn main() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let owner_info: cosmwasm_std::MessageInfo = mock_info("onwer000", &[]);
    let user_info: cosmwasm_std::MessageInfo = mock_info("user000", &[]);
    let _token_info: cosmwasm_std::MessageInfo = mock_info("token000", &[]);

    let gate_info: cosmwasm_std::MessageInfo = mock_info("gate_contract", &[]);

    let remote_market_contract: Addr = Addr::unchecked("remote_market_contract");

    let remote_chain = "injective".to_string();

    // INIT CONTRACT

    let msg = InstantiateMsg {};

    let _res = instantiate(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();

    // DEPOSIT A TOKEN

    // let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
    //     sender: user_info.sender.to_string(),
    //     amount: Uint128::from(100_u128),
    //     msg: to_binary(&Cw20MsgType::Deposit {}).unwrap(),
    // });

    // let _res = execute(deps.as_mut(), env.clone(), token_info.clone(), msg).unwrap();

    // DEPOSIT A NATIVE

    let msg = ExecuteMsg::Deposit {};

    let _res = execute(
        deps.as_mut(),
        env.clone(),
        mock_info(
            user_info.sender.as_ref(),
            &[Coin {
                denom: "uAtom".to_string(),
                amount: Uint128::from(100_u128),
            }],
        ),
        msg,
    )
    .unwrap();

    // QUERY POSITION

    let msg = QueryMsg::Position {
        user: user_info.sender.clone(),
    };

    let res = query(deps.as_ref(), env.clone(), msg).unwrap();

    let res: Position = from_binary(&res).unwrap();

    println!("{:?}", res);

    // REGISTER GATE

    let msg = ExecuteMsg::RegisterGate {
        contract: gate_info.sender.clone(),
    };

    let _res = execute(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();

    // SET PERMISSION

    let msg = ExecuteMsg::GateSetPermission {
        contract: remote_market_contract.to_string(),
        chain: remote_chain.clone(),
    };

    execute(deps.as_mut(), env.clone(), owner_info, msg).unwrap();

    // BRIDGE POSITION

    let msg = ExecuteMsg::ReceiveGateMsg(GateMsg::CollectRequests {
        sender: user_info.sender,
        msg: to_binary(&GateCollectMsgsAllowed::BridgePosition {
            to_remote_addr: "remote000".to_string(),
            chain: remote_chain,
            native_info: Some(msgs::NativeInfo {
                path_middle_forward: vec![],
                dest_denom: "ibc/uatom".to_string(),
                channel_id: "channel-1".to_string(),
                timeout: None,
            }),
        })
        .unwrap(),
    });

    let res = execute(
        deps.as_mut(),
        env.clone(),
        mock_info(
            gate_info.sender.as_str(),
            &[Coin {
                denom: "uluna".to_string(),
                amount: Uint128::from(50_u128),
            }],
        ),
        msg,
    )
    .unwrap();

    println!("{:?}", res);

    // RECEVIE A MSG FROM GATE

    let mut collaterals: HashMap<String, Uint128> = HashMap::new();

    collaterals.insert("token_1".to_string(), Uint128::from(500_u128));

    let msg = ExecuteMsg::ReceiveGateMsg(GateMsg::ReceivedMsg {
        sender: remote_market_contract.to_string(),
        msg: to_binary(&BridgeMsgInfo {
            sender: "remote_user".to_string(),
            receiver: "local_user".to_string(),
            dest_position: Position {
                loan: Uint128::from(500_u128),
                collaterals: collaterals.clone(),
            },
            src_position: Position {
                loan: Uint128::from(500_u128),
                collaterals,
            },
        })
        .unwrap(),
    });

    let res = execute(deps.as_mut(), env.clone(), gate_info, msg).unwrap();

    println!("{:?}", res);

    let msg = QueryMsg::Position {
        user: Addr::unchecked("local_user"),
    };

    let res = query(deps.as_ref(), env, msg).unwrap();

    let res: Position = from_binary(&res).unwrap();

    println!("{:?}", res)
}
