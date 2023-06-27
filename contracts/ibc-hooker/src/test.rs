use cosmwasm_std::{
    from_binary,
    testing::{mock_dependencies, mock_env, mock_info},
    Coin,
};

use crate::{
    contract::{execute, instantiate, query, sudo},
    msgs::{
        ExecuteMsg, Forward, IBCLifecycleComplete, InstantiateMsg, QueryMsg, QueryResponse, SudoMsg,
    },
};

#[test]
fn main() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    let src_channel = "src_channel".to_string();

    let msg = InstantiateMsg {};

    let receiver_ibc_hooker = "receiver_ibc_hooker".to_string();

    let to_address = "receiver_addr".to_string();

    let sender_addr = "sender_addr".to_string();

    let _res = instantiate(deps.as_mut(), env.clone(), mock_info("owner", &[]), msg).unwrap();

    let msg = ExecuteMsg::Request {
        to_fail: false,
        receiver_ibc_hooker,
        channel_id: src_channel,
        to_address: to_address.clone(),
        // forward: None
        forward: Some(Forward {
            receiver: "middle_contract_addr".to_string(),
            channel: "middle_channel".to_string(),
        }),
    };

    let _res = execute(
        deps.as_mut(),
        env.clone(),
        mock_info("sender", &[Coin::new(100_u128, "ucoin")]),
        msg,
    )
    .unwrap();

    let msg = ExecuteMsg::ReceivedRequest {
        to_fail: false,
        from_address: sender_addr.clone(),
        to_address: to_address.clone(),
    };

    let _res = execute(
        deps.as_mut(),
        env.clone(),
        mock_info("sender", &[Coin::new(100_u128, "ucoin")]),
        msg,
    )
    .unwrap();

    let msg = ExecuteMsg::ReceivedRequest {
        to_fail: false,
        from_address: sender_addr,
        to_address,
    };

    let _res = execute(
        deps.as_mut(),
        env.clone(),
        mock_info("sender", &[Coin::new(50_u128, "ucoin")]),
        msg,
    )
    .unwrap();

    let res = query(deps.as_ref(), env.clone(), QueryMsg::State {});

    let _query_res: QueryResponse = from_binary(&res.unwrap()).unwrap();

    // println!("{:?}", query_res);

    let msg = SudoMsg::IBCLifecycleComplete(IBCLifecycleComplete::IBCAck {
        channel: "channel_1".to_string(),
        sequence: 1,
        ack: "ok".to_string(),
        success: true,
    });

    let _res = sudo(deps.as_mut(), env.clone(), msg).unwrap();

    let res = query(deps.as_ref(), env, QueryMsg::State {});

    let _query_res: QueryResponse = from_binary(&res.unwrap()).unwrap();

    // println!("{:?}", query_res);
}
