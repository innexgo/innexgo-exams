use super::handlers;
use super::utils;
use super::Config;
use super::Db;
use super::SERVICE_NAME;
use auth_service_api::client::AuthService;
use innexgo_hours_api::response::InnexgoHoursError;
use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use warp::http::StatusCode;
use warp::Filter;

/// Helper to combine the multiple filters together with Filter::or, possibly boxing the types in
/// the process. This greatly helps the build times for `ipfs-http`.
/// https://github.com/seanmonstar/warp/issues/507#issuecomment-615974062
macro_rules! combine {
  ($x:expr, $($y:expr),+) => {{
      let filter = ($x).boxed();
      $( let filter = (filter.or($y)).boxed(); )+
      filter
  }}
}

/// The function that will show all ones to call
pub fn api(
  config: Config,
  db: Db,
  auth_service: AuthService,
) -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
  // public API
  api_info()
    .or(combine!(
      adapter(
        config.clone(),
        db.clone(),
        auth_service.clone(),
        warp::path!("public" / "test" / "test"),
        handlers::test
      ),
      adapter(
        config.clone(),
        db.clone(),
        auth_service.clone(),
        warp::path!("public" / "test" / "test2"),
        handlers::test,
      )
    ))
    .recover(handle_rejection)
}

fn api_info() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
  let mut info = HashMap::new();
  info.insert("version", "0.1");
  info.insert("name", SERVICE_NAME);
  warp::path!("info").map(move || warp::reply::json(&info))
}

// this function adapts a handler function to a warp filter
// it accepts an initial path filter
fn adapter<PropsType, ResponseType, F>(
  config: Config,
  db: Db,
  auth_service: AuthService,
  filter: impl Filter<Extract = (), Error = warp::Rejection> + Clone,
  handler: fn(Config, Db, AuthService, PropsType) -> F,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
where
  F: Future<Output = Result<ResponseType, InnexgoHoursError>> + Send,
  PropsType: Send + serde::de::DeserializeOwned,
  ResponseType: Send + serde::ser::Serialize,
{
  // lets you pass in an arbitrary parameter
  fn with<T: Clone + Send>(t: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone {
    warp::any().map(move || t.clone())
  }

  filter
    .and(with((config, db, auth_service)))
    .and(warp::body::json())
    .and_then(async move |(config, db, auth_service), props| {
      handler(config, db, auth_service, props)
        .await
        .map_err(innexgo_hours_error)
    })
    .map(|x| warp::reply::json(&Ok::<_, ()>(x)))
}

// This function receives a `Rejection` and tries to return a custom
// value, otherwise simply passes the rejection along.
async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
  let code;
  let message;

  if err.is_not_found() {
    code = StatusCode::NOT_FOUND;
    message = InnexgoHoursError::NotFound;
  } else if err
    .find::<warp::filters::body::BodyDeserializeError>()
    .is_some()
  {
    message = InnexgoHoursError::DecodeError;
    code = StatusCode::BAD_REQUEST;
  } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
    code = StatusCode::METHOD_NOT_ALLOWED;
    message = InnexgoHoursError::MethodNotAllowed;
  } else if let Some(InnexgoHoursErrorRejection(innexgo_hours_error)) = err.find() {
    code = StatusCode::BAD_REQUEST;
    message = innexgo_hours_error.clone();
  } else {
    // We should have expected this... Just log and say its a 500
    utils::log(utils::Event {
      msg: "intercepted unknown error kind".to_owned(),
      source: format!("{:#?}", err),
      severity: utils::SeverityKind::Error,
    });
    code = StatusCode::INTERNAL_SERVER_ERROR;
    message = InnexgoHoursError::Unknown;
  }

  Ok(warp::reply::with_status(
    warp::reply::json(&Err::<(), _>(message)),
    code,
  ))
}

// This type represents errors that we can generate
// These will be automatically converted to a proper string later
#[derive(Debug)]
pub struct InnexgoHoursErrorRejection(pub InnexgoHoursError);
impl warp::reject::Reject for InnexgoHoursErrorRejection {}

fn innexgo_hours_error(innexgo_hours_error: InnexgoHoursError) -> warp::reject::Rejection {
  warp::reject::custom(InnexgoHoursErrorRejection(innexgo_hours_error))
}
