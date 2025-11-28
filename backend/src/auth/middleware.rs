use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpMessage};
use actix_web::error::ErrorUnauthorized;
use futures::future::{LocalBoxFuture, ready, Ready};
use std::rc::Rc;
use std::task::{Context, Poll};
use uuid::Uuid;
use crate::auth::jwt::verify_jwt;

pub struct JwtAuth;

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;
    
    fn new_transform(&self, service: S) -> Ready<Result<Self::Transform, Self::InitError>> {
        ready(Ok(JwtAuthMiddleware { service: Rc::new(service) }))
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;
    
    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }
    
    fn call(&self, req: ServiceRequest) -> Self::Future {
        
        let path = req.path().to_string();
        if path.starts_with("/auth/") || path == "signin" || path == "signup" {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await });
        }
        
        let header = req
            .headers()
            .get(actix_web::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());
        
        if header.is_none() {
            return Box::pin(async move { Err(ErrorUnauthorized("Missing authorization header")) });
        }
        
        let header = header.unwrap();
        if !header.to_lowercase().starts_with("bearer ") {
            return Box::pin(async move { Err(ErrorUnauthorized("Invalid authorization header format")) });
        }
        
        let token = header[7..].trim();
        
        match verify_jwt(token) {
            Ok(data) => {
                match Uuid::parse_str(&data.claims.sub) {
                    Ok(uid) => {
                        req.extensions_mut().insert(uid);
                        let svc = self.service.clone();
                        Box::pin(async move {
                            let res = svc.call(req).await?;
                            Ok(res)
                        })
                    }
                    Err(_) => {
                        Box::pin(async move { Err(ErrorUnauthorized("Invalid JWT token")) })
                    }
                }
            }
            Err(_) => {
                Box::pin(async move { Err(ErrorUnauthorized("JWT verification failed")) })
            }
        }
    }
}