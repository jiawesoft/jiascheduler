use anyhow::{anyhow, Result};
use bytes::{BufMut, BytesMut};

use super::msg::Msg;

pub struct Protocol {}

impl Protocol {
    const REQ_MARK: u8 = 0;
    const RESP_MARK: u8 = 1;

    pub fn is_response(data: &Vec<u8>) -> bool {
        data[0] == Self::RESP_MARK
    }

    pub fn pack_request(data: Msg) -> Vec<u8> {
        let mut b = BytesMut::new();
        b.put_u8(Self::REQ_MARK);
        b.extend(serde_json::to_vec(&data).unwrap());
        b.to_vec()
    }

    pub fn unpack_request(data: Vec<u8>) -> Result<Msg> {
        if data[0] != Self::REQ_MARK {
            return Err(anyhow!("invalid request msg format"));
        }
        let val = &data[1..];
        Ok(serde_json::from_slice::<Msg>(val)?)
    }

    pub fn pack_response(data: Msg) -> Vec<u8> {
        let mut b = BytesMut::new();
        b.put_u8(Self::RESP_MARK);
        let data = serde_json::to_vec(&data).unwrap();
        b.extend(data);
        b.to_vec()
    }

    pub fn unpack_response(data: Vec<u8>) -> Result<Msg> {
        if data[0] != Self::RESP_MARK {
            return Err(anyhow!("invalid response msg format"));
        }
        let val = &data[1..];
        Ok(serde_json::from_slice::<Msg>(val)?)
    }
}

#[test]
fn pack_request() {
    use crate::bridge::msg::MsgKind;
    use serde_json::json;
    let old = Msg {
        id: 12,
        data: MsgKind::Request(crate::bridge::msg::MsgReqKind::PullJobRequest(
            json!({"hello":"world"}),
        )),
    };

    let data = Protocol::pack_request(old.clone());

    match Protocol::unpack_request(data) {
        Ok(new) => {
            assert!(old == new, "a:{:?}, b:{:?} not equal", old, new,)
        }
        Err(_) => todo!(),
    }
}
