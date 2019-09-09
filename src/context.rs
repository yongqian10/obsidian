use futures::{Future, Stream};
use hyper::{header::HeaderValue, Body, HeaderMap, Method, Request, Uri};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::str::FromStr;
use url::form_urlencoded;

use crate::router::{from_cow_map, Params, FormError};

/// Context is the interface for accessing request and request data
#[derive(Debug)]
pub struct Context {
    request: Request<Body>,
    params_data: Params,
}

impl Context {
    pub fn new(request: Request<Body>, params_data: Params) -> Self {
        Context {
            request,
            params_data,
        }
    }

    /// Access request header
    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        self.request.headers()
    }

    /// Access mutable request header
    pub fn headers_mut(&mut self) -> &mut HeaderMap<HeaderValue> {
        self.request.headers_mut()
    }

    /// Access request method
    pub fn method(&self) -> &Method {
        self.request.method()
    }

    /// Access request uri
    pub fn uri(&self) -> &Uri {
        self.request.uri()
    }

    /// Method to get the params value according to key. 
    /// Panic if key is not found.
    /// 
    /// # Example
    /// 
    /// ```
    /// # use obsidian::StatusCode;
    /// # use obsidian::context::Context;
    /// # use obsidian::router::ResponseBuilder;
    /// 
    /// // Assumming ctx contains params for id and mode
    /// fn get_handler(ctx: Context, res: ResponseBuilder) -> ResponseBuilder {
    ///     let id: i32 = ctx.param("id").unwrap();
    ///     let mode: String = ctx.param("mode").unwrap();
    /// 
    ///     assert_eq!(id, 1);
    ///     assert_eq!(mode, "edit".to_string());
    /// 
    ///     res.status(StatusCode::OK)
    /// }
    /// 
    /// ```
    /// 
    pub fn param<T: FromStr>(&self, key: &str) -> Result<T, T::Err>
    {
        self.params_data.get_params(key).unwrap().parse()
    }

    /// Method to get the forms query data from the request body. 
    /// Body is consumed after calling this method.
    /// Untagged is not supported
    /// 
    /// # Example
    /// ```
    /// # use serde_derive::*;
    /// 
    /// # use obsidian::context::Context;
    /// # use obsidian::router::ResponseBuilder;
    /// # use obsidian::StatusCode;
    /// 
    /// #[derive(Deserialize, Serialize, Debug)]
    /// struct FormResult {
    ///     id: i32,
    ///     mode: String,
    /// }
    /// 
    /// // Assume ctx contains form query with data {id=1&mode=edit}
    /// fn get_handler(mut ctx: Context, res: ResponseBuilder) -> ResponseBuilder {
    ///     let result: FormResult = ctx.form().unwrap();
    /// 
    ///     assert_eq!(result.id, 1);
    ///     assert_eq!(result.mode, "edit".to_string());
    /// 
    ///     res.status(StatusCode::OK)
    /// }
    /// ```
    pub fn form<T: DeserializeOwned>(&mut self) -> Result<T, FormError> {
        let body = self.take_body();

        let chunks = match body.concat2().wait() {
            Ok(chunk) => chunk,
            Err(e) => {
                println!("{}", e);
                hyper::Chunk::default()
            }
        };

        let mut parsed_form_map: HashMap<String, Vec<String>> = HashMap::default();
        let mut cow_form_map = HashMap::default();

        // Parse and merge chunks with same name key
        form_urlencoded::parse(&chunks)
            .into_owned()
            .for_each(|(key, val)| {
                parsed_form_map.entry(key).or_insert(vec![]).push(val);
            });

        // Wrap vec with cow pointer
        parsed_form_map.iter().for_each(|(key, val)| {
            cow_form_map
                .entry(std::borrow::Cow::from(key))
                .or_insert(std::borrow::Cow::from(val));
        });

        Ok(from_cow_map(&cow_form_map)?)
    }

    /// Form value merge with Params
    pub fn form_with_param<T: DeserializeOwned>(&mut self) -> Result<T, ()> {
        unimplemented!()
    }

    /// Method to get the json data from the request body. Body is consumed after calling this method.
    /// The result can be either handled by using static type or dynamic map.
    /// Panic if parsing fail.
    /// 
    /// # Example
    /// 
    /// ### Handle by static type
    /// ```
    /// # use serde_derive::*;
    /// 
    /// # use obsidian::context::Context;
    /// # use obsidian::router::ResponseBuilder;
    /// # use obsidian::StatusCode;
    /// 
    /// #[derive(Deserialize, Serialize, Debug)]
    /// struct JsonResult {
    ///     id: i32,
    ///     mode: String,
    /// }
    /// 
    /// // Assume ctx contains json with data {id:1, mode:'edit'}
    /// fn get_handler(mut ctx: Context, res: ResponseBuilder) -> ResponseBuilder {
    ///     let result: JsonResult = ctx.json().unwrap();
    /// 
    ///     assert_eq!(result.id, 1);
    ///     assert_eq!(result.mode, "edit".to_string());
    /// 
    ///     res.status(StatusCode::OK)
    /// }
    /// ```
    /// 
    /// ### Handle by dynamic map
    /// ```
    /// # use serde_json::Value;
    /// 
    /// # use obsidian::context::Context;
    /// # use obsidian::router::ResponseBuilder;
    /// # use obsidian::StatusCode;
    /// 
    /// // Assume ctx contains json with data {id:1, mode:'edit'}
    /// fn get_handler(mut ctx: Context, res: ResponseBuilder) -> ResponseBuilder {
    ///     let result: serde_json::Value = ctx.json().unwrap();
    /// 
    ///     assert_eq!(result["id"], 1);
    ///     assert_eq!(result["mode"], "edit".to_string());
    /// 
    ///     res.status(StatusCode::OK)
    /// }
    /// ```
    pub fn json<T: DeserializeOwned>(&mut self) -> Result<T, serde_json::error::Error> {
        let body = self.take_body();

        let chunks = match body.concat2().wait() {
            Ok(chunk) => chunk,
            Err(e) => {
                println!("{}", e);
                hyper::Chunk::default()
            }
        };

        Ok(serde_json::from_slice(&chunks)?)
    }

    /// Json value merged with Params
    pub fn json_with_param<T: DeserializeOwned>(&mut self) -> Result<T, serde_json::error::Error> {
        unimplemented!()
    }

    /// Consumes body of the request and replace it with empty body.
    pub fn take_body(&mut self) -> Body {
        std::mem::replace(self.request.body_mut(), Body::empty())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::router::Params;
    use hyper::{Request, Body};
    use serde_derive::*;
    use serde_json::json;

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct FormResult {
        id: i32,
        mode: String,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct FormExtraResult {
        id: i32,
        mode: String,
        #[serde(default)]
        extra: i32,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct JsonResult {
        id: i32,
        mode: String,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct JsonExtraResult {
        id: i32,
        mode: String,
        #[serde(default)]
        extra: i32,
    }

    #[test]
    fn test_params() {
        let mut params_map = HashMap::default();

        params_map.insert("id".to_string(), "1".to_string());
        params_map.insert("mode".to_string(), "edit".to_string());

        let params = Params::new(params_map);
        let request = Request::new(Body::from(""));

        let ctx = Context::new(request, params);

        let id: i32 = ctx.param("id").unwrap();
        let mode: String = ctx.param("mode").unwrap();

        assert_eq!(id, 1);
        assert_eq!(mode, "edit".to_string());
    }

    #[test]
    #[should_panic]
    fn test_params_without_value() {
        let mut params_map = HashMap::default();

        params_map.insert("mode".to_string(), "edit".to_string());

        let params = Params::new(params_map);
        let request = Request::new(Body::from(""));

        let ctx = Context::new(request, params);

        let _mode: String = ctx.param("mode").unwrap();
        let _id: i32 = ctx.param("id").unwrap();
    }

    #[test]
    fn test_form() {
        let params = Params::new(HashMap::default());
        let request = Request::new(Body::from("id=1&mode=edit"));

        let mut ctx = Context::new(request, params);

        let actual_result: FormResult = ctx.form().unwrap();
        let expected_result = FormResult{id: 1, mode: "edit".to_string()};

        assert_eq!(actual_result, expected_result);
    }

    #[test]
    fn test_form_with_extra_body() {
        let params = Params::new(HashMap::default());
        let request = Request::new(Body::from("id=1&mode=edit&extra=true"));

        let mut ctx = Context::new(request, params);

        let actual_result: FormResult = ctx.form().unwrap();
        let expected_result = FormResult{id: 1, mode: "edit".to_string()};

        assert_eq!(actual_result, expected_result);
    }

    #[test]
    fn test_form_with_extra_field() {
        let params = Params::new(HashMap::default());
        let request = Request::new(Body::from("id=1&mode=edit"));

        let mut ctx = Context::new(request, params);

        let actual_result: FormExtraResult = ctx.form().unwrap();
        let expected_result = FormExtraResult{id: 1, mode: "edit".to_string(), extra: i32::default()};

        assert_eq!(actual_result, expected_result);
    }

    #[test]
    fn test_json_struct() {
        let params = Params::new(HashMap::default());
        let request = Request::new(Body::from("{\"id\":1,\"mode\":\"edit\"}"));

        let mut ctx = Context::new(request, params);

        let actual_result: JsonResult = ctx.json().unwrap();
        let expected_result = JsonResult{id: 1, mode: "edit".to_string()};

        assert_eq!(actual_result, expected_result);
    }

    #[test]
    fn test_json_value() {
        let params = Params::new(HashMap::default());
        let request = Request::new(Body::from("{\"id\":1,\"mode\":\"edit\"}"));

        let mut ctx = Context::new(request, params);

        let actual_result: serde_json::Value = ctx.json().unwrap();

        assert_eq!(actual_result["id"], json!(1));
        assert_eq!(actual_result["mode"], json!("edit"));
    }
}
