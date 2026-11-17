//! Manual check against the real API: `LINEAR_API_KEY=... scripts/cargo.sh run -p linear_api --example smoke`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("LINEAR_API_KEY").map_err(|_| "set LINEAR_API_KEY")?;
    let client = linear_api::Client::new(key);

    let viewer = client.viewer()?;
    println!("viewer: {} ({}) in {}", viewer.name, viewer.id, viewer.organization_name);

    let catalog = client.catalog("smoke")?;
    println!("teams: {}", catalog.teams.len());

    let page = client.issues_updated_since("smoke", None)?;
    println!("issues: {}", page.issues.len());
    for issue in page.issues.iter().take(5) {
        println!("  {} {}", issue.identifier, issue.title);
    }
    println!("rate limit remaining: {:?}", client.rate_limit_remaining());
    Ok(())
}
