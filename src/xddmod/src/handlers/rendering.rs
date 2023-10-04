use minijinja::context;
use minijinja::Environment;
use serde::Serialize;

use crate::handlers::persistence::Reply;

#[derive(thiserror::Error, Debug)]
pub enum RenderingError {
    #[error("empty rendered reply {reply:?} with template env {template_env:?}")]
    EmptyRenderedReply { reply: Reply, template_env: String },
    #[error(transparent)]
    Templating(#[from] minijinja::Error),
}

impl Reply {
    pub fn render_template<S: Serialize>(
        &self,
        template_env: &Environment,
        ctx: Option<&S>,
    ) -> Result<String, RenderingError> {
        let ctx = ctx.map_or_else(|| context!(), |ctx| minijinja::value::Value::from_serializable(ctx));
        let rendered_reply: String = template_env.render_str(&self.template, ctx).map(|s| s.trim().into())?;

        if rendered_reply.is_empty() {
            return Err(RenderingError::EmptyRenderedReply {
                reply: self.clone(),
                template_env: format!("{:?}", template_env),
            });
        }

        Ok(rendered_reply)
    }
}
