use constant_time_eq::constant_time_eq;
use ntex::http::header::{AUTHORIZATION, HeaderValue};
use ntex::web::{WebRequest, WebResponse};
use ntex::{Ctx, Middleware, Service, web};

use crate::state::AppState;

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

impl<S> Middleware<S, AppState> for SatoriAuthorization {
    type Service = SatoriAuthorizationService<S>;

    fn create(&self, state: &AppState, service: S) -> Self::Service {
        let prefix = &state.settings.path;
        let prefix = prefix.strip_prefix("/").unwrap_or(prefix);
        let prefix = prefix.strip_suffix("/").unwrap_or(prefix);
        SatoriAuthorizationService {
            service,
            prefix: prefix.to_owned(),
        }
    }
}

pub struct SatoriAuthorizationService<S> {
    service: S,
    prefix: String,
}

impl<S, ReqSt> Service<AppState, WebRequest<ReqSt>> for SatoriAuthorizationService<S>
where
    S: Service<AppState, WebRequest<ReqSt>, Res = WebResponse>,
{
    type Res = WebResponse;
    type Error = S::Error;

    ntex::forward_ready!(AppState, service);
    ntex::forward_shutdown!(AppState, service);

    async fn call(
        &self,
        req: WebRequest<ReqSt>,
        ctx: Ctx<'_, Self, AppState>,
    ) -> Result<Self::Res, Self::Error> {
        let state = ctx.st();
        let path = &req.path();
        let path = path.strip_prefix("/").unwrap_or(path);
        println!("{} {}", path, self.prefix);
        let path = match path.strip_prefix(&self.prefix) {
            Some(path) => path,
            None => return ctx.call(&self.service, req).await,
        };
        let headers = req.headers();
        let is_events = path == "v1/events";
        let is_meta = path.starts_with("v1/meta/");
        let is_proxy = path.starts_with("v1/proxy/");
        if !is_events && !is_proxy && !state.settings.token.is_empty() {
            match headers.get(AUTHORIZATION) {
                Some(value) if check_authorization(value, &state.settings.token) => (),
                Some(_) => return Ok(unauthorized(req, "Invalid authorization header.")),
                None => return Ok(unauthorized(req, "No authorization header provided.")),
            }
        }
        if !is_events && !is_meta && !is_proxy {
            match headers.get("Satori-Platform") {
                Some(value) if value == "telegram" => (),
                Some(_) => return Ok(unauthorized(req, "Invalid platform header.")),
                None => return Ok(unauthorized(req, "No platform header provided.")),
            }
            let self_id = state
                .self_info_cache
                .lock()
                .await
                .get_id()
                .bot_api_dialog_id()
                .unwrap_or_default();
            match headers.get("Satori-User-ID") {
                Some(value) if value.to_str().map(|s| s.parse::<i64>()) == Ok(Ok(self_id)) => (),
                Some(_) => return Ok(unauthorized(req, "Invalid user ID header.")),
                None => return Ok(unauthorized(req, "No user ID header provided.")),
            }
        }
        ctx.call(&self.service, req).await
    }
}
