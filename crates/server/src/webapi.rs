use crate::agentdb::Agent;
use crate::Server;
use http_types::headers::HeaderValue;
use std::sync::Arc;
use tide::security::{CorsMiddleware, Origin};
use tide::{prelude::*, Body, Error, Request, StatusCode};
use uuid::Uuid;

pub struct WebApi {}

impl WebApi {
    pub async fn start_api(ctx: &Arc<Server>) {
        let mut app = tide::with_state(ctx.clone());

        let cors = CorsMiddleware::new()
            .allow_methods("GET, POST, OPTIONS".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*"))
            .allow_credentials(false);

        app.with(cors);

        app.at("/agent")
            .get(Self::list_agent)
            .post(Self::create_agent);

        app.at("/agent/:agent_id")
            .get(Self::get_agent_config)
            .put(Self::put_agent_config)
            .delete(Self::delete_agent);

        app.at("/agent/:agent_id/task")
            .get(Self::list_agent_tasks)
            .post(Self::create_agent_task);

        app.at("/agent/:agent_id/task/:task_id")
            .get(Self::get_agent_task)
            .put(Self::put_agent_task)
            .delete(Self::delete_agent_task);

        app.listen(&ctx.api_addr).await.expect("Failed to bind");
    }

    fn get_param<'a, T: From<&'a str>>(req: &'a Request<Arc<Server>>, k: &str) -> tide::Result<T> {
        req.param(k)
            .map(|v| v.into())
            .map_err(|_| Error::from_str(StatusCode::BadRequest, format!("need {}", k)))
    }

    async fn get_agent_config(req: Request<Arc<Server>>) -> tide::Result {
        // parse params
        let agent_id = Self::get_param(&req, "agent_id")?;
        let agent_id = Uuid::parse_str(agent_id).status(StatusCode::BadRequest)?;

        // get agent
        let agents = req.state().agents.read().await;
        let agent = agents.get_agent(&agent_id).status(StatusCode::NotFound)?;

        Ok(Body::from_json(&agent.config)?.into())
    }

    async fn list_agent(req: Request<Arc<Server>>) -> tide::Result {
        let agent_id = req.state().agents.read().await.list_agents();

        Ok(Body::from_json(&agent_id)?.into())
    }

    async fn create_agent(mut req: Request<Arc<Server>>) -> tide::Result {
        let agent: Agent = req.body_json().await?;

        // insert agent
        let mut agents = req.state().agents.write().await;
        let agent_id = agents.insert_config(agent);

        Ok(Body::from_string(agent_id.to_string()).into())
    }

    async fn put_agent_config(mut req: Request<Arc<Server>>) -> tide::Result {
        let agent_id = Self::get_param(&req, "agent_id")?;
        let agent_id = Uuid::parse_str(agent_id).status(StatusCode::BadRequest)?;

        let agent: Agent = req.body_json().await?;

        req.state()
            .agents
            .write()
            .await
            .update_config(&agent_id, agent)
            .status(StatusCode::NotFound)?;

        Ok(StatusCode::Ok.into())
    }

    async fn delete_agent(req: Request<Arc<Server>>) -> tide::Result {
        let agent_id = Self::get_param(&req, "agent_id")?;
        let agent_id = Uuid::parse_str(agent_id).status(StatusCode::BadRequest)?;

        // insert agent
        let mut agents = req.state().agents.write().await;
        agents.remove(&agent_id).status(StatusCode::NotFound)?;

        Ok(StatusCode::Ok.into())
    }

    async fn get_agent_task(req: Request<Arc<Server>>) -> tide::Result {
        // parse params
        let agent_id = Self::get_param(&req, "agent_id")?;
        let agent_id = Uuid::parse_str(agent_id).status(StatusCode::BadRequest)?;

        // get task_id
        let task_id = Self::get_param(&req, "task_id")?;
        let task_id = Uuid::parse_str(task_id).status(StatusCode::BadRequest)?;

        // get agent
        let agents = req.state().agents.read().await;
        let agent = agents.get_agent(&agent_id).status(StatusCode::BadRequest)?;
        let task = agent.tasks.get(&task_id).status(StatusCode::BadRequest)?;

        Ok(Body::from_json(task)?.into())
    }

    async fn list_agent_tasks(req: Request<Arc<Server>>) -> tide::Result {
        // parse params
        let agent_id = Self::get_param(&req, "agent_id")?;
        let agent_id = Uuid::parse_str(agent_id).status(StatusCode::BadRequest)?;

        // get agent
        let agents = req.state().agents.read().await;
        let agent = agents.get_agent(&agent_id).status(StatusCode::BadRequest)?;

        Ok(Body::from_json(&agent.tasks)?.into())
    }

    async fn create_agent_task(mut req: Request<Arc<Server>>) -> tide::Result {
        // parse params
        let agent_id = Self::get_param(&req, "agent_id")?;
        let agent_id = Uuid::parse_str(agent_id).status(StatusCode::BadRequest)?;

        // get task
        let task = req.body_json().await?;

        // get agent
        let mut agents = req.state().agents.write().await;
        let res = agents
            .insert_agent_task(&agent_id, task)
            .status(StatusCode::BadRequest)?;

        Ok(Body::from_string(res.to_string()).into())
    }

    async fn put_agent_task(mut req: Request<Arc<Server>>) -> tide::Result {
        // parse params
        let agent_id = Self::get_param(&req, "agent_id")?;
        let agent_id = Uuid::parse_str(agent_id).status(StatusCode::BadRequest)?;

        // get task_id
        let task_id = Self::get_param(&req, "task_id")?;
        let task_id = Uuid::parse_str(task_id).status(StatusCode::BadRequest)?;

        // get task
        let task = req.body_json().await?;

        // get agent
        let mut agent = req.state().agents.write().await;
        agent
            .update_agent_task(&agent_id, &task_id, task)
            .ok_or_else(|| {
                Error::from_str(StatusCode::BadRequest, "invalid agent id or task id")
            })?;

        // And respond with the new JSON.
        Ok(StatusCode::Ok.into())
    }

    async fn delete_agent_task(req: Request<Arc<Server>>) -> tide::Result {
        // parse params
        let agent_id = Self::get_param(&req, "agent_id")?;
        let agent_id = Uuid::parse_str(agent_id).status(StatusCode::BadRequest)?;

        // get task_id
        let task_id = Self::get_param(&req, "task_id")?;
        let task_id = Uuid::parse_str(task_id).status(StatusCode::BadRequest)?;

        // get agent
        let mut agent = req.state().agents.write().await;
        agent
            .remove_agent_task(&agent_id, &task_id)
            .ok_or_else(|| {
                Error::from_str(StatusCode::BadRequest, "invalid agent id or task id")
            })?;

        // And respond with the new JSON.
        Ok(StatusCode::Ok.into())
    }
}
