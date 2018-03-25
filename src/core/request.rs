use hyper::{Body, Headers, HttpVersion, Method, Uri};
use std::{fmt, net};
use tokio_core::reactor::Handle;
use recognizer::Params;
use anymap::AnyMap;
use serde_json::{self, from_slice, from_value};
use hyper::Chunk;
use serde::de::DeserializeOwned;
use queryst_prime::{self, parse};

/// The Request Struct, This is passed to Middlewares and route handlers.
/// 
pub struct Request {
	pub(crate) uri: Uri,
	pub(crate) handle: Option<Handle>,
	pub(crate) body: Option<Body>,
	pub(crate) version: HttpVersion,
	pub(crate) headers: Headers,
	pub(crate) remote: Option<net::SocketAddr>,
	pub(crate) method: Method,
	pub(crate) anyMap: AnyMap,
}

#[derive(Debug)]
pub enum JsonError {
	None,
	Err(serde_json::Error),
}

#[derive(Debug)]
pub enum QueryParseError {
	SerdeError(serde_json::Error),
	ParseError(queryst_prime::ParseError),
	None,
}

impl Request {
	pub(crate) fn new(
		method: Method,
		uri: Uri,
		version: HttpVersion,
		headers: Headers,
		body: Body,
	) -> Self {
		Self {
			method,
			uri,
			version,
			headers,
			body: Some(body),
			remote: None,
			anyMap: AnyMap::new(),
			handle: None,
		}
	}

	pub fn reactor_handle(&self) -> Handle {
		self.handle.clone().unwrap()
	}

	/// returns a reference to the request's HttpVersion
	#[inline]
	pub fn version(&self) -> &HttpVersion {
		&self.version
	}

	/// returns a reference to the request's headers
	#[inline]
	pub fn headers(&self) -> &Headers {
		&self.headers
	}

	/// returns a reference to the request's method
	#[inline]
	pub fn method(&self) -> &Method {
		&self.method
	}

	/// returns a request to the request's Uri
	#[inline]
	pub fn uri(&self) -> &Uri {
		&self.uri
	}

	/// returns the query path
	#[inline]
	pub fn path(&self) -> &str {
		self.uri.path()
	}

	/// returns the IP of the connected client.
	#[inline]
	pub fn remote_ip(&self) -> net::SocketAddr {
		self.remote.unwrap()
	}

	/// Serializes the query string into a struct via serde.
	///
	///  # Examples
	///
	/// ```rust,ignore
	/// [derive(Serialize, Deserialize)]
	/// struct AccessToken {
	/// 	token: String,
	/// }
	///
	/// pub fn login(req: Request, _res: Response) {
	/// 	if let AccessToken { token } = req.query::<AccessToken>() {
	/// 		// do something with the token here.
	/// 	}
	/// }
	/// ```
	/// returns `None` if the query could not be serialized.
	/// Ideally this should return a `Result<T, serde_json::Error>`
	/// It would be corrected in a later version.
	///

	#[inline]
	pub fn query<T>(&self) -> Result<T, QueryParseError>
	where
		T: DeserializeOwned,
	{
		self
			.uri
			.query()
			.ok_or(QueryParseError::None)
			.and_then(|query| parse(query).map_err(QueryParseError::ParseError))
			.and_then(|value| from_value::<T>(value).map_err(QueryParseError::SerdeError))
	}

	/// Get the url params for the request
	///
	/// e.g `/profile/:id`
	///
	/// ```rust,ignore
	/// [service]
	/// pub fn ProfileService(req: Request, res: Response) {
	/// 	let profileId = req.params().unwrap()["id"];
	/// 	// Its safe to unwrap here as this woute would never be matched without the `id`
	/// }
	/// ```
	pub fn params(&self) -> Option<&Params> {
		self.anyMap.get::<Params>()
	}

	/// The request struct constains an `AnyMap` so that middlewares can append additional
	/// information.
	///
	/// you can get values out of the `AnyMap` by using this method.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// [derive(Serialize, Deserialize)]
	/// struct AccessToken {
	/// 	token: String,
	/// }
	///
	/// struct User {
	/// 	name: String,
	/// }
	///
	/// [middleware(Request)]
	/// pub fn AssertAuth(req: Request) {
	/// 	if let AccessToken { token } = req.query::<AccessToken>() {
	/// 		if let user = db::fetchUser::<User>(token) {
	/// 			// pseudo code
	/// 			req.set::<User>(user); // Set the user
	/// 		} else {
	/// 			return Err((404, "User Not Found!").into());
	/// 		}
	/// 	} else {
	/// 		return Err((401, "Unauthorized!").into());
	/// 	}
	/// }
	///
	/// [service]
	/// pub fn ProfileService(req: Request, res: Response) {
	/// 	let user = req.get::<User>().unwrap();
	/// 	// Its safe to unwrap here, because if user isn't set this service will never
	/// 	// be called.
	/// }
	/// ```
	pub fn get<T: 'static>(&self) -> Option<&T> {
		self.anyMap.get::<T>()
	}

	pub fn get_owned<T: 'static>(&mut self) -> Option<T> {
		self.anyMap.remove::<T>()
	}

	/// same as above.
	pub fn set<T: 'static>(&mut self, value: T) -> Option<T> {
		self.anyMap.insert::<T>(value)
	}

	/// move the request body. note that this takes ownership of `self`, use
	/// wisely.
	#[inline]
	pub fn body(self) -> Body {
		self.body.unwrap_or_default()
	}

	/// serialize the request's json value into a struct
	///
	/// Note that the json value needs to have been previously set on the request by a middleware.
	/// otherwise this would return `Err(JsonError::None)`
	pub fn json<T>(&self) -> Result<T, JsonError>
	where
		T: DeserializeOwned,
	{
		match self.get::<Chunk>() {
			Some(ref slice) => from_slice::<T>(slice).map_err(JsonError::Err),
			_ => Err(JsonError::None),
		}
	}

	/// get a reference to the body
	#[inline]
	pub fn body_ref(&self) -> Option<&Body> {
		self.body.as_ref()
	}

	/// set the request body
	pub fn set_body(&mut self, body: Body) {
		self.body = Some(body)
	}

	/// Decontruct this request.
	pub fn deconstruct(self) -> (Method, Uri, HttpVersion, Headers, Body) {
		(
			self.method,
			self.uri,
			self.version,
			self.headers,
			self.body.unwrap_or_default(),
		)
	}
}

impl fmt::Debug for Request {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_struct("Request")
			.field("method", &self.method)
			.field("uri", &self.uri)
			.field("version", &self.version)
			.field("remote", &self.remote)
			.field("headers", &self.headers)
			.finish()
	}
}
