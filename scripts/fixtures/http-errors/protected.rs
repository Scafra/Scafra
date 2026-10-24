use scafra::prelude::*;

#[controller("/protected", authenticated)]
pub struct Protected;
#[routes]
impl Protected {
    #[get("/")]
    pub async fn get(&self) -> &'static str {
        "ok"
    }
    #[get("/role")]
    #[roles("admin")]
    pub async fn role(&self) -> &'static str {
        "admin"
    }
    #[get("/scope")]
    #[scopes("data.read")]
    pub async fn scope(&self) -> &'static str {
        "scoped"
    }
    #[get("/public")]
    #[public]
    pub async fn public(&self) -> &'static str {
        "public"
    }
}
