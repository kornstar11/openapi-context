//! Module for combining hyper services

use std::{io, fmt};
use hyper::server::{Service, NewService};
use hyper::{Request, Response, StatusCode};
use futures::{future, Future};
use context::Context;

/// Trait for getting the path of a request
pub trait HasPath {
    /// Retrieve the path
    fn path(&self) -> &str;
}

impl HasPath for Request {
    fn path(&self) -> &str {
        self.path()
    }
}

impl HasPath for (Request, Context) {
    fn path(&self) -> &str {
        self.0.path()
    }
}

/// Trait for generating a default "not found" response
pub trait HasNotFound {
    /// Return a "not found" response
    fn not_found() -> Self;
}

impl HasNotFound for Response {
    fn not_found() -> Self {
        Response::new().with_status(StatusCode::NotFound)
    }
}

/// Trait for wrapping hyper NewServices to uniformize the return type of new_service().
/// This is necessary in order for the NewServices with different Instance types to
/// be stored in a single collection.
pub trait BoxedNewService<U, V, W> {
    /// Create a new Service trait object
    fn boxed_new_service(
        &self,
    ) -> Result<
        Box<
            Service<
                Request = U,
                Response = V,
                Error = W,
                Future = Box<Future<Item = V, Error = W>>,
            >,
        >,
        io::Error,
    >;
}

impl<T, U, V, W> BoxedNewService<U, V, W> for T
where
    T: NewService<Request = U, Response = V, Error = W>,
    T::Instance: Service<Future = Box<Future<Item = V, Error = W>>>
        + 'static,
{
    /// Call the new_service() method of the wrapped NewService and Box the result
    fn boxed_new_service(
        &self,
    ) -> Result<
        Box<
            Service<
                Request = U,
                Response = V,
                Error = W,
                Future = Box<Future<Item = V, Error = W>>,
            >,
        >,
        io::Error,
    > {
        let service = self.new_service()?;
        Ok(Box::new(service))
    }
}

/// A struct combining multiple NewServices. The NewServices are stored in a list
/// in the order they are added, together with an associated base path. The service
/// generated by the CompositeNewService will pass a request to the first service
/// in the list whose base path is a prefix of the request's path or return a
/// "not found" response if there is no match.
pub struct CompositeNewService<U, V, W>(Vec<(&'static str, Box<BoxedNewService<U, V, W>>)>)
where
    U: HasPath,
    V: HasNotFound + 'static,
    W: 'static;

impl<U: HasPath, V: HasNotFound, W> CompositeNewService<U, V, W> {
    /// Create an empty CompositeNewService
    pub fn new() -> Self {
        CompositeNewService(Vec::new())
    }

    /// Add a new NewService with a base path to the composite
    pub fn append_new_service(
        &mut self,
        base_path: &'static str,
        new_service: Box<BoxedNewService<U, V, W>>,
    ) {
        self.0.push((base_path, new_service));
    }
}

impl<U, V, W> NewService for CompositeNewService<U, V, W>
where
    U: HasPath,
    V: HasNotFound + 'static,
    W: 'static,
{
    type Request = U;
    type Response = V;
    type Error = W;
    type Instance = CompositeService<U, V, W>;

    fn new_service(&self) -> Result<Self::Instance, io::Error> {
        let mut vec = Vec::new();

        for &(base_path, ref new_service) in self.0.iter() {
            vec.push((base_path, new_service.boxed_new_service()?))
        }

        Ok(CompositeService(vec))
    }
}

impl<U, V, W> fmt::Debug for CompositeNewService<U, V, W>
where
    U: HasPath,
    V: HasNotFound + 'static,
    W: 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        // Get vector of base paths
        let str_vec: Vec<&'static str> = self.0.iter().map(|&(base_path, _)| base_path).collect();
        write!(
            f,
            "CompositeNewService accepting base paths: {:?}",
            str_vec,
        )
    }
}

/// A struct combining multiple hyper Services
pub struct CompositeService<U, V, W>(
    Vec<
        (&'static str,
         Box<
            Service<
                Request = U,
                Response = V,
                Error = W,
                Future = Box<Future<Item = V, Error = W>>,
            >,
        >),
    >
)
where
    U: HasPath,
    V: HasNotFound + 'static,
    W: 'static;

impl<U, V, W> Service for CompositeService<U, V, W>
where
    U: HasPath,
    V: HasNotFound + 'static,
    W: 'static,
{
    type Request = U;
    type Response = V;
    type Error = W;
    type Future = Box<Future<Item = V, Error = W>>;

    fn call(&self, req: Self::Request) -> Self::Future {

        let mut result = None;

        for &(base_path, ref service) in self.0.iter() {
            if req.path().starts_with(base_path) {
                result = Some(service.call(req));
                break;
            }
        }

        if let Some(result) = result {
            result
        } else {
            Box::new(future::ok(V::not_found()))
        }
    }
}

impl<U, V, W> fmt::Debug for CompositeService<U, V, W>
where
    U: HasPath,
    V: HasNotFound + 'static,
    W: 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        // Get vector of base paths
        let str_vec: Vec<&'static str> = self.0.iter().map(|&(base_path, _)| base_path).collect();
        write!(
            f,
            "CompositeService accepting base paths: {:?}",
            str_vec,
        )
    }
}
