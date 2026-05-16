#[get("/feed")]
pub async fn feed_handler(hb: WebTemplates) -> impl Responder {
    render(
        hb,
        "feed",
        json!({ "user": "Guest", "data": "your feed goes here" }),
        HttpResponse::Ok(),
    )
}
