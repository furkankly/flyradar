use reqwest::{Client, RequestBuilder as ReqwestRequestBuilder};

#[derive(Clone, Debug)]
pub struct RequestBuilderMachines(RequestBuilder);
#[derive(Clone, Debug)]
pub struct RequestBuilderGraphql(RequestBuilder);
#[derive(Clone, Debug)]
pub struct RequestBuilderFly(RequestBuilder);

#[derive(Clone, Debug)]
struct RequestBuilder {
    http_client: Client,
    /// These fields won't change after init, don't need to Arc them
    base_url: String,
    access_token: String,
}

impl RequestBuilder {
    pub fn new(http_client: Client, base_url: String, access_token: String) -> Self {
        RequestBuilder {
            http_client,
            base_url,
            access_token,
        }
    }
}

impl RequestBuilderMachines {
    pub fn new(http_client: Client, base_url: String, access_token: String) -> Self {
        RequestBuilderMachines(RequestBuilder::new(http_client, base_url, access_token))
    }
    pub fn get(&self, path: String) -> ReqwestRequestBuilder {
        self.0
            .http_client
            .get(format!("{}{path}", self.0.base_url))
            .bearer_auth(&self.0.access_token)
    }
    pub fn post(&self, path: String) -> ReqwestRequestBuilder {
        self.0
            .http_client
            .post(format!("{}{path}", self.0.base_url))
            .bearer_auth(&self.0.access_token)
    }
    pub fn delete(&self, path: String) -> ReqwestRequestBuilder {
        self.0
            .http_client
            .delete(format!("{}{path}", self.0.base_url))
            .bearer_auth(&self.0.access_token)
    }
}

impl RequestBuilderGraphql {
    pub fn new(http_client: Client, base_url: String, access_token: String) -> Self {
        RequestBuilderGraphql(RequestBuilder::new(http_client, base_url, access_token))
    }
    pub fn query(&self) -> ReqwestRequestBuilder {
        self.0
            .http_client
            .post(&self.0.base_url)
            .bearer_auth(&self.0.access_token)
    }
}

impl RequestBuilderFly {
    pub fn new(http_client: Client, base_url: String, access_token: String) -> Self {
        RequestBuilderFly(RequestBuilder::new(http_client, base_url, access_token))
    }
    pub fn get(&self, path: String) -> ReqwestRequestBuilder {
        self.0
            .http_client
            .get(format!("{}{path}", self.0.base_url))
            .bearer_auth(&self.0.access_token)
    }
}

pub fn find_err(err: &(dyn std::error::Error + 'static), pattern: &str) -> bool {
    let mut err = Some(err);
    while let Some(e) = err {
        if e.to_string().contains(pattern) {
            return true;
        }
        err = e.source();
    }
    false
}
