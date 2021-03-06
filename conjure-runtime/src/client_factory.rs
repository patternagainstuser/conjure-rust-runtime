// Copyright 2020 Palantir Technologies, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use crate::blocking;
use crate::client::ClientState;
use crate::config::{ServiceConfig, ServicesConfig};
use crate::{Client, HostMetricsRegistry, Idempotency, ServerQos, ServiceError, UserAgent};
use arc_swap::ArcSwap;
use conjure_error::Error;
use refreshable::Refreshable;
use std::sync::Arc;
use witchcraft_metrics::MetricRegistry;

/// A factory type which can create clients that will live-reload in response to configuration updates.
#[derive(Clone)]
pub struct ClientFactory {
    config: Arc<Refreshable<ServicesConfig, Error>>,
    user_agent: Option<UserAgent>,
    metrics: Option<Arc<MetricRegistry>>,
    host_metrics: Option<Arc<HostMetricsRegistry>>,
    server_qos: Option<ServerQos>,
    service_error: Option<ServiceError>,
    idempotency: Option<Idempotency>,
}

impl ClientFactory {
    /// Creates a new client factory based off of a refreshable `ServicesConfig`.
    pub fn new(config: Refreshable<ServicesConfig, Error>) -> ClientFactory {
        ClientFactory {
            config: Arc::new(config),
            user_agent: None,
            metrics: None,
            host_metrics: None,
            server_qos: None,
            service_error: None,
            idempotency: None,
        }
    }

    /// Sets the user agent sent by clients.
    ///
    /// Required.
    pub fn user_agent(&mut self, user_agent: UserAgent) -> &mut Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Sets clients' behavior in response to a QoS error from the server.
    ///
    /// Defaults to `ServerQos::AutomaticRetry`.
    pub fn server_qos(&mut self, server_qos: ServerQos) -> &mut Self {
        self.server_qos = Some(server_qos);
        self
    }

    /// Sets clients' behavior in response to a service error from the server.
    ///
    /// Defaults to `ServiceError::WrapInNewError`.
    pub fn service_error(&mut self, service_error: ServiceError) -> &mut Self {
        self.service_error = Some(service_error);
        self
    }

    /// Sets clients' behavior to determine if a request is idempotent or not.
    ///
    /// Only idempotent requests will be retried.
    ///
    /// Defaults to `Idempotency::ByMethod`.
    pub fn idempotency(&mut self, idempotency: Idempotency) -> &mut Self {
        self.idempotency = Some(idempotency);
        self
    }

    /// Sets the metric registry used to register client metrics.
    ///
    /// Defaults to no registry.
    pub fn metrics(&mut self, metrics: Arc<MetricRegistry>) -> &mut Self {
        self.metrics = Some(metrics);
        self
    }

    /// Sets the host metrics registry used to track host performance.
    ///
    /// Defaults to no registry.
    pub fn host_metrics(&mut self, host_metrics: Arc<HostMetricsRegistry>) -> &mut Self {
        self.host_metrics = Some(host_metrics);
        self
    }

    /// Creates a new client for the specified service.
    ///
    /// The client's configuration will automatically refresh to track changes in the factory's `ServicesConfiguration`.
    ///
    /// If no configuration is present for the specified service in the `ServicesConfiguration`, the client will
    /// immediately return an error for all requests.
    pub fn client(&self, service: &str) -> Result<Client, Error> {
        let service_config = self.config.map({
            let service = service.to_string();
            move |c| c.merged_service(&service).unwrap_or_default()
        });

        let user_agent = self.user_agent.clone();
        let metrics = self.metrics.clone();
        let host_metrics = self.host_metrics.clone();
        let server_qos = self.server_qos;
        let service_error = self.service_error;
        let idempotency = self.idempotency;

        let make_state = move |config: &ServiceConfig| {
            let mut builder = Client::builder();
            builder.from_config(config);
            if let Some(user_agent) = user_agent.clone() {
                builder.user_agent(user_agent);
            }
            if let Some(metrics) = metrics.clone() {
                builder.metrics(metrics);
            }
            if let Some(host_metrics) = host_metrics.clone() {
                builder.host_metrics(host_metrics);
            }
            if let Some(server_qos) = server_qos {
                builder.server_qos(server_qos);
            }
            if let Some(service_error) = service_error {
                builder.service_error(service_error);
            }
            if let Some(idempotency) = idempotency {
                builder.idempotency(idempotency);
            }

            ClientState::new(&builder)
        };

        let state = make_state(&service_config.get())?;
        let state = Arc::new(ArcSwap::new(Arc::new(state)));

        let subscription = service_config.subscribe({
            let state = state.clone();
            move |config| {
                let new_state = make_state(config)?;
                state.store(Arc::new(new_state));
                Ok(())
            }
        })?;

        Ok(Client::new(state, Some(subscription)))
    }

    /// Creates a new blocking client for the specified service.
    ///
    /// The client's configuration will automatically refresh to track changes in the factory's `ServicesConfiguration`.
    ///
    /// If no configuration is present for the specified service in the `ServicesConfiguration`, the client will
    /// immediately return an error for all requests.
    pub fn blocking_client(&self, service: &str) -> Result<blocking::Client, Error> {
        self.client(service).map(blocking::Client)
    }
}
