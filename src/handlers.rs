use super::Db;
use auth_service_api::client::AuthService;
use auth_service_api::response::AuthError;
use auth_service_api::response::User;

use innexgo_hours_api::request;
use innexgo_hours_api::response;

use super::utils;

use std::error::Error;

use super::Config;

pub async fn get_user_if_api_key_valid(
  auth_service: &auth_service_api::client::AuthService,
  api_key: String,
) -> Result<User, response::InnexgoHoursError> {
  auth_service
    .get_user_by_api_key_if_valid(api_key)
    .await
    .map_err(report_auth_err)
}

fn report_postgres_err(e: tokio_postgres::Error) -> response::InnexgoHoursError {
  utils::log(utils::Event {
    msg: e.to_string(),
    source: e.source().map(|e| e.to_string()),
    severity: utils::SeverityKind::Error,
  });
  response::InnexgoHoursError::InternalServerError
}

fn report_auth_err(e: AuthError) -> response::InnexgoHoursError {
  match e {
    AuthError::ApiKeyNonexistent => response::InnexgoHoursError::ApiKeyUnauthorized,
    AuthError::ApiKeyUnauthorized => response::InnexgoHoursError::ApiKeyNonexistent,
    c => {
      let ae = match c {
        AuthError::InternalServerError => response::InnexgoHoursError::AuthInternalServerError,
        AuthError::MethodNotAllowed => response::InnexgoHoursError::AuthBadRequest,
        AuthError::BadRequest => response::InnexgoHoursError::AuthBadRequest,
        _ => response::InnexgoHoursError::AuthOther,
      };

      utils::log(utils::Event {
        msg: ae.as_ref().to_owned(),
        source: Some(format!("auth service: {}", c.as_ref())),
        severity: utils::SeverityKind::Error,
      });

      ae
    }
  }
}

pub async fn test(
  _config: Config,
  db: Db,
  auth_service: AuthService,
  props: request::SubscriptionNewProps,
) -> Result<response::Subscription, response::InnexgoHoursError> {
  // validate api key
  let user = get_user_if_api_key_valid(&auth_service, props.api_key).await?;
  let con = &mut *db.lock().await;
  // create event
  let subscription = subscription_service::add(con, user.user_id, props.subscription_kind, 1, 0)
    .await
    .map_err(report_postgres_err)?;
  // return json
  fill_subscription(con, subscription).await
}

pub async fn test2(
  _config: Config,
  db: Db,
  auth_service: AuthService,
  props: request::SubscriptionNewProps,
) -> Result<response::Subscription, response::InnexgoHoursError> {
  // validate api key
  let user = get_user_if_api_key_valid(&auth_service, props.api_key).await?;
  let con = &mut *db.lock().await;
  // create event
  let subscription = subscription_service::add(con, user.user_id, props.subscription_kind, 1, 0)
    .await
    .map_err(report_postgres_err)?;
  // return json
  fill_subscription(con, subscription).await
}

