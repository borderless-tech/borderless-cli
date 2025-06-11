use anyhow::Result;
use cliclack::{confirm, input, intro, log::info, outro, select};
use url::Url;

use crate::api::{Link, LinkDb};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Existing(Link),
    Create,
}

pub fn handle_link() -> Result<()> {
    intro("🔗 Creating or modifying links to external nodes")?;

    // Get existing links
    let db = LinkDb::open()?;

    // Select link to modify or create new link
    let mut selectable: Vec<_> = db.get_links().into_iter().map(Item::Existing).collect();
    selectable.push(Item::Create);

    let mut prompt = select("Select existing link or create new one");
    for item in selectable {
        let (label, hint) = match &item {
            Item::Existing(link) => (
                link.name.clone(),
                format!("Contract-Node - {} - <SECRET_API_KEY>", link.api),
            ),
            Item::Create => (
                "Create new".to_string(),
                "link a new node to the cli".to_string(),
            ),
        };
        prompt = prompt.item(item, label, hint);
    }
    let selection = prompt.filter_mode().interact()?;

    match selection {
        Item::Existing(link) => {
            modify_existing(db, link)?;
        }
        Item::Create => {
            create_new(db)?;
        }
    };
    Ok(())
}

fn create_new(mut db: LinkDb) -> Result<()> {
    info("Creating a new link...")?;

    // NOTE: This is not very efficient, but its good enough for now.
    let db_copy = db.clone();

    let name: String = input("Enter a name for this connection:")
        .placeholder("my-node")
        // there are some lifetime issues when using a &db here;
        // but I don't have the time now for a clean solution..
        .validate(move |input: &String| {
            if input.is_empty() {
                Err("Name cannot be empty")
            } else if db_copy.contains(input.as_str()) {
                Err("The name already exists in our db")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let api: Url = input("Enter the API base-url:")
        .placeholder("http://localhost:3000")
        .validate(|input: &String| match input.parse::<Url>() {
            Ok(url) => {
                if url.cannot_be_a_base() {
                    Err(
                        "url cannot be a base-url - required form: http[s]://<AUTHORIY>[:PORT]"
                            .to_string(),
                    )
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(e.to_string()),
        })
        .interact()?;

    let api_key: String = input("Enter the API-key for the connection (leave empty if none):")
        .placeholder("sk-d67e0cca1ab6d95f243")
        .validate(|input: &String| {
            if input.find(char::is_whitespace).is_some() {
                Err("whitespaces are not allowed in API-keys")
            } else {
                Ok(())
            }
        })
        .required(false)
        .default_input("")
        .interact()?;

    let api_key = if api_key.is_empty() {
        None
    } else {
        Some(api_key)
    };

    let new_link = Link { name, api, api_key };
    info(&new_link.to_string())?;

    // Save to db
    db.add_link(new_link);
    db.commit()?;
    info("Saved new link to db. You can now use this connection with the cli-tool.")?;

    Ok(())
}

fn modify_existing(mut db: LinkDb, link: Link) -> Result<()> {
    info(format!("Changing existing link {}", link.to_string()))?;
    let delete = select("What do you want to do?")
        .item(true, "Delete link", "deletes the node from our database")
        .item(
            false,
            "Modify link",
            "changes values like API-address or API-key",
        )
        .interact()?;

    if delete {
        if confirm(format!(
            "Delete {} ? This cannot be undone!",
            link.to_string()
        ))
        .interact()?
        {
            db.remove_link(&link.name)?;
            db.commit()?;
            outro(format!("Removed link '{}'", link.name))?;
        } else {
            outro("Abort by user. Nothing changed.")?;
        }
        return Ok(());
    }

    let api: Url = input("Enter the API base-url (leave empty to keep the current value):")
        .placeholder(&link.api.to_string())
        .validate(|input: &String| {
            if let Err(e) = input.parse::<Url>() {
                Err(e.to_string())
            } else {
                Ok(())
            }
        })
        .default_input(&link.api.to_string())
        .required(false)
        .interact()?;

    let api_key: String =
        input("Enter the API-key for the connection (leave empty to keep the current value):")
            .placeholder(&link.api_key.clone().unwrap_or_default())
            .validate(|input: &String| {
                if input.find(char::is_whitespace).is_some() {
                    Err("whitespaces are not allowed in API-keys")
                } else {
                    Ok(())
                }
            })
            .default_input(&link.api_key.unwrap_or_default())
            .required(false)
            .interact()?;

    let api_key = if api_key.is_empty() {
        None
    } else {
        Some(api_key)
    };

    let new_link = Link {
        name: link.name.clone(),
        api,
        api_key,
    };

    // Commit changes
    db.modify_link(&link.name, new_link)?;
    db.commit()?;

    outro(format!("Modified link '{}'", link.name))?;
    Ok(())
}
