use lightning::impl_writeable_msg;
use lightning::ln::wire;
use serde::de;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;

const LSPS_MESSAGE_SERIALIZED_STRUCT_NAME: &str = "LSPSMessage";
const JSONRPC_FIELD_KEY: &str = "jsonrpc";
const JSONRPC_FIELD_VALUE: &str = "2.0";
const JSONRPC_METHOD_FIELD_KEY: &str = "method";
const JSONRPC_ID_FIELD_KEY: &str = "id";
const JSONRPC_PARAMS_FIELD_KEY: &str = "params";
const JSONRPC_RESULT_FIELD_KEY: &str = "result";
const JSONRPC_ERROR_FIELD_KEY: &str = "error";
const JSONRPC_INVALID_MESSAGE_ERROR_CODE: i32 = -32700;
const JSONRPC_INVALID_MESSAGE_ERROR_MESSAGE: &str = "parse error";
const LSPS0_LISTPROTOCOLS_METHOD_NAME: &str = "lsps0.list_protocols";

pub const LSPS_MESSAGE_TYPE: u16 = 37913;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLSPSMessage {
	pub payload: String,
}

impl_writeable_msg!(RawLSPSMessage, { payload }, {});

impl wire::Type for RawLSPSMessage {
	fn type_id(&self) -> u16 {
		LSPS_MESSAGE_TYPE
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ResponseError {
	pub code: i32,
	pub message: String,
	pub data: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ListProtocolsRequest {}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ListProtocolsResponse {
	pub protocols: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LSPS0Request {
	ListProtocols(ListProtocolsRequest),
}

impl LSPS0Request {
	pub fn method(&self) -> &str {
		match self {
			LSPS0Request::ListProtocols(_) => LSPS0_LISTPROTOCOLS_METHOD_NAME,
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LSPS0Response {
	ListProtocols(ListProtocolsResponse),
	ListProtocolsError(ResponseError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LSPS0Message {
	Request(RequestId, LSPS0Request),
	Response(RequestId, LSPS0Response),
}

impl TryFrom<LSPSMessage> for LSPS0Message {
	type Error = ();

	fn try_from(message: LSPSMessage) -> Result<Self, Self::Error> {
		match message {
			LSPSMessage::Invalid => Err(()),
			LSPSMessage::LSPS0(message) => Ok(message),
		}
	}
}

impl From<LSPS0Message> for LSPSMessage {
	fn from(message: LSPS0Message) -> Self {
		LSPSMessage::LSPS0(message)
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LSPSMessage {
	Invalid,
	LSPS0(LSPS0Message),
}

impl LSPSMessage {
	pub fn from_str_with_id_map(
		json_str: &str, request_id_to_method: &mut HashMap<String, String>,
	) -> Result<Self, serde_json::Error> {
		let deserializer = &mut serde_json::Deserializer::from_str(json_str);
		let visitor = LSPSMessageVisitor { request_id_to_method };
		deserializer.deserialize_any(visitor)
	}

	pub fn get_request_id_and_method(&self) -> Option<(String, String)> {
		match self {
			LSPSMessage::LSPS0(LSPS0Message::Request(request_id, request)) => {
				Some((request_id.0.clone(), request.method().to_string()))
			}
			_ => None,
		}
	}
}

impl Serialize for LSPSMessage {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let mut jsonrpc_object =
			serializer.serialize_struct(LSPS_MESSAGE_SERIALIZED_STRUCT_NAME, 3)?;

		jsonrpc_object.serialize_field(JSONRPC_FIELD_KEY, JSONRPC_FIELD_VALUE)?;

		match self {
			LSPSMessage::LSPS0(LSPS0Message::Request(request_id, request)) => {
				jsonrpc_object.serialize_field(JSONRPC_METHOD_FIELD_KEY, request.method())?;
				jsonrpc_object.serialize_field(JSONRPC_ID_FIELD_KEY, &request_id.0)?;

				match request {
					LSPS0Request::ListProtocols(params) => {
						jsonrpc_object.serialize_field(JSONRPC_PARAMS_FIELD_KEY, params)?
					}
				};
			}
			LSPSMessage::LSPS0(LSPS0Message::Response(request_id, response)) => {
				jsonrpc_object.serialize_field(JSONRPC_ID_FIELD_KEY, &request_id.0)?;

				match response {
					LSPS0Response::ListProtocols(result) => {
						jsonrpc_object.serialize_field(JSONRPC_RESULT_FIELD_KEY, result)?;
					}
					LSPS0Response::ListProtocolsError(error) => {
						jsonrpc_object.serialize_field(JSONRPC_ERROR_FIELD_KEY, error)?;
					}
				}
			}
			LSPSMessage::Invalid => {
				let error = ResponseError {
					code: JSONRPC_INVALID_MESSAGE_ERROR_CODE,
					message: JSONRPC_INVALID_MESSAGE_ERROR_MESSAGE.to_string(),
					data: None,
				};

				jsonrpc_object.serialize_field(JSONRPC_ID_FIELD_KEY, &serde_json::Value::Null)?;
				jsonrpc_object.serialize_field(JSONRPC_ERROR_FIELD_KEY, &error)?;
			}
		}

		jsonrpc_object.end()
	}
}

struct LSPSMessageVisitor<'a> {
	request_id_to_method: &'a mut HashMap<String, String>,
}

impl<'de, 'a> Visitor<'de> for LSPSMessageVisitor<'a> {
	type Value = LSPSMessage;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("JSON-RPC object")
	}

	fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
	where
		A: MapAccess<'de>,
	{
		let mut id: Option<String> = None;
		let mut method: Option<&str> = None;
		let mut params = None;
		let mut result = None;
		let mut error: Option<ResponseError> = None;

		while let Some(key) = map.next_key()? {
			match key {
				"id" => {
					id = Some(map.next_value()?);
				}
				"method" => {
					method = Some(map.next_value()?);
				}
				"params" => {
					params = Some(map.next_value()?);
				}
				"result" => {
					result = Some(map.next_value()?);
				}
				"error" => {
					error = Some(map.next_value()?);
				}
				_ => {
					let _: serde_json::Value = map.next_value()?;
				}
			}
		}

		match (id, method) {
			(Some(id), Some(method)) => match method {
				LSPS0_LISTPROTOCOLS_METHOD_NAME => {
					self.request_id_to_method.insert(id.clone(), method.to_string());

					Ok(LSPSMessage::LSPS0(LSPS0Message::Request(
						RequestId(id),
						LSPS0Request::ListProtocols(ListProtocolsRequest {}),
					)))
				}
				_ => Err(de::Error::custom(format!(
					"Received request with unknown method: {}",
					method
				))),
			},
			(Some(id), None) => match self.request_id_to_method.get(&id) {
				Some(method) => match method.as_str() {
					LSPS0_LISTPROTOCOLS_METHOD_NAME => {
						if let Some(error) = error {
							Ok(LSPSMessage::LSPS0(LSPS0Message::Response(
								RequestId(id),
								LSPS0Response::ListProtocolsError(error),
							)))
						} else if let Some(result) = result {
							let list_protocols_response =
								serde_json::from_value(result).map_err(de::Error::custom)?;
							Ok(LSPSMessage::LSPS0(LSPS0Message::Response(
								RequestId(id),
								LSPS0Response::ListProtocols(list_protocols_response),
							)))
						} else {
							Err(de::Error::custom("Received invalid JSON-RPC object: one of method, result, or error required"))
						}
					}
					_ => Err(de::Error::custom(format!(
						"Received response for an unknown request method: {}",
						method
					))),
				},
				None => Err(de::Error::custom(format!(
					"Received response for unknown request id: {}",
					id
				))),
			},
			(None, Some(method)) => {
				Err(de::Error::custom(format!("Received unknown notification: {}", method)))
			}
			(None, None) => Err(de::Error::custom(
				"Received invalid JSON-RPC object: one of method or id required",
			)),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn deserializes_request() {
		let json = r#"{
			"jsonrpc": "2.0",
			"id": "request:id:xyz123",
			"method": "lsps0.list_protocols"
		}"#;

		let mut request_id_method_map = HashMap::new();

		let msg = LSPSMessage::from_str_with_id_map(json, &mut request_id_method_map);
		assert!(msg.is_ok());
		let msg = msg.unwrap();
		assert_eq!(
			msg,
			LSPSMessage::LSPS0(LSPS0Message::Request(
				RequestId("request:id:xyz123".to_string()),
				LSPS0Request::ListProtocols(ListProtocolsRequest {})
			))
		);
	}

	#[test]
	fn serializes_request() {
		let request = LSPSMessage::LSPS0(LSPS0Message::Request(
			RequestId("request:id:xyz123".to_string()),
			LSPS0Request::ListProtocols(ListProtocolsRequest {}),
		));
		let json = serde_json::to_string(&request).unwrap();
		assert_eq!(
			json,
			r#"{"jsonrpc":"2.0","method":"lsps0.list_protocols","id":"request:id:xyz123","params":{}}"#
		);
	}

	#[test]
	fn deserializes_success_response() {
		let json = r#"{
	        "jsonrpc": "2.0",
	        "id": "request:id:xyz123",
	        "result": {
	            "protocols": [1,2,3]
	        }
	    }"#;
		let mut request_id_to_method_map = HashMap::new();
		request_id_to_method_map
			.insert("request:id:xyz123".to_string(), "lsps0.list_protocols".to_string());

		let response =
			LSPSMessage::from_str_with_id_map(json, &mut request_id_to_method_map).unwrap();

		assert_eq!(
			response,
			LSPSMessage::LSPS0(LSPS0Message::Response(
				RequestId("request:id:xyz123".to_string()),
				LSPS0Response::ListProtocols(ListProtocolsResponse { protocols: vec![1, 2, 3] })
			))
		);
	}

	#[test]
	fn deserializes_error_response() {
		let json = r#"{
	        "jsonrpc": "2.0",
	        "id": "request:id:xyz123",
	        "error": {
	            "code": -32617,
				"message": "Unknown Error"
	        }
	    }"#;
		let mut request_id_to_method_map = HashMap::new();
		request_id_to_method_map
			.insert("request:id:xyz123".to_string(), "lsps0.list_protocols".to_string());

		let response =
			LSPSMessage::from_str_with_id_map(json, &mut request_id_to_method_map).unwrap();

		assert_eq!(
			response,
			LSPSMessage::LSPS0(LSPS0Message::Response(
				RequestId("request:id:xyz123".to_string()),
				LSPS0Response::ListProtocolsError(ResponseError {
					code: -32617,
					message: "Unknown Error".to_string(),
					data: None
				})
			))
		);
	}

	#[test]
	fn deserialize_fails_with_unknown_request_id() {
		let json = r#"{
	        "jsonrpc": "2.0",
	        "id": "request:id:xyz124",
	        "result": {
	            "protocols": [1,2,3]
	        }
	    }"#;
		let mut request_id_to_method_map = HashMap::new();
		request_id_to_method_map
			.insert("request:id:xyz123".to_string(), "lsps0.list_protocols".to_string());

		let response = LSPSMessage::from_str_with_id_map(json, &mut request_id_to_method_map);
		assert!(response.is_err());
	}

	#[test]
	fn serializes_response() {
		let response = LSPSMessage::LSPS0(LSPS0Message::Response(
			RequestId("request:id:xyz123".to_string()),
			LSPS0Response::ListProtocols(ListProtocolsResponse { protocols: vec![1, 2, 3] }),
		));
		let json = serde_json::to_string(&response).unwrap();
		assert_eq!(
			json,
			r#"{"jsonrpc":"2.0","id":"request:id:xyz123","result":{"protocols":[1,2,3]}}"#
		);
	}
}
