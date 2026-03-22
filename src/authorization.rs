use std::sync::Arc;

use constant_time_eq::constant_time_eq;
use ntex::http::header::{AUTHORIZATION, HeaderValue};
use ntex::{Middleware, Service, ServiceCtx, web};
use tokio::sync::Mutex;

use crate::self_info_cache::SelfInfoCache;
use crate::settings::Settings;

fn check_authorization(authorization: &HeaderValue, token: &str) -> bool {
    if let Some(authorization) = authorization.as_bytes().strip_prefix(b"Bearer ") {
        constant_time_eq(authorization, token.as_bytes())
    } else {
        false
    }
}

fn unauthorized<Err>(req: web::WebRequest<Err>, detail: &'static str) -> web::WebResponse {
    web::WebResponse::new(
        web::HttpResponse::Unauthorized().body(detail),
        req.into_parts().0,
    )
}

pub struct SatoriAuthorization;

impl<S, C> Middleware<S, C> for SatoriAuthorization {
    type Service = SatoriAuthorizationMiddleware<S>;

    fn create(&self, service: S, _: C) -> Self::Service {
        SatoriAuthorizationMiddleware { service }
    }
}

pub struct SatoriAuthorizationMiddleware<S> {
    service: S,
}

impl<S, Err> Service<web::WebRequest<Err>> for SatoriAuthorizationMiddleware<S>
where
    S: Service<web::WebRequest<Err>, Response = web::WebResponse, Error = web::Error>,
{
    type Response = web::WebResponse;
    type Error = web::Error;

    async fn call(
        &self,
        req: web::WebRequest<Err>,
        ctx: ServiceCtx<'_, Self>,
    ) -> Result<Self::Response, Self::Error> {
        let settings = req
            .app_state::<Arc<Settings>>()
            .expect("Settings is not initalized");
        let self_info_manager = req
            .app_state::<Arc<Mutex<SelfInfoCache>>>()
            .expect("SelfInfoManager is not initalized");
        let prefix = settings.path.trim_end_matches("/");
        let path = match req.path().strip_prefix(prefix) {
            Some(path) => path,
            None => return ctx.call(&self.service, req).await,
        };
        let headers = req.headers();
        let is_events = path == "/v1/events";
        let is_meta = path.starts_with("/v1/meta/");
        let is_proxy = path.starts_with("/v1/proxy/");
        if !is_events && !is_proxy && !settings.token.is_empty() {
            match headers.get(AUTHORIZATION) {
                Some(value) if check_authorization(value, &settings.token) => (),
                Some(_) => return Ok(unauthorized(req, "Invalid authorization header.")),
                None => return Ok(unauthorized(req, "No authorization header provided.")),
            }
        }
        if !is_events && !is_meta && !is_proxy {
            match headers
                .get("Satori-Platform")
                .or_else(|| headers.get("X-Platform"))
            {
                Some(value) if value == "telegram" => (),
                Some(_) => return Ok(unauthorized(req, "Invalid platform header.")),
                None => return Ok(unauthorized(req, "No platform header provided.")),
            }
            let self_id = self_info_manager.lock().await.get_id().bot_api_dialog_id();
            match headers
                .get("Satori-User-ID")
                .or_else(|| headers.get("X-Self-ID"))
            {
                Some(value) if value.to_str().map(|s| s.parse::<i64>()) == Ok(Ok(self_id)) => (),
                Some(_) => return Ok(unauthorized(req, "Invalid user ID header.")),
                None => return Ok(unauthorized(req, "No user ID header provided.")),
            }
        }
        ctx.call(&self.service, req).await
    }
}
