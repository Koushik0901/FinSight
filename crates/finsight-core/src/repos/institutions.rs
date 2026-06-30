//! CRUD for financial institutions.

use crate::error::CoreResult;
use crate::models::{Institution, NewInstitution};
use rusqlite::{params, Connection};

pub fn upsert(conn: &mut Connection, input: NewInstitution) -> CoreResult<Institution> {
    conn.execute(
        "INSERT INTO institutions (id, name, domain, sfin_url) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, domain = excluded.domain, sfin_url = excluded.sfin_url",
        params![&input.id, &input.name, &input.domain, &input.sfin_url],
    )?;
    Ok(Institution {
        id: input.id,
        name: input.name,
        domain: input.domain,
        sfin_url: input.sfin_url,
    })
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Institution> {
    conn.query_row(
        "SELECT id, name, domain, sfin_url FROM institutions WHERE id = ?1",
        params![id],
        |r| {
            Ok(Institution {
                id: r.get(0)?,
                name: r.get(1)?,
                domain: r.get(2)?,
                sfin_url: r.get(3)?,
            })
        },
    )
    .map_err(Into::into)
}

pub fn list(conn: &mut Connection) -> CoreResult<Vec<Institution>> {
    let mut stmt =
        conn.prepare("SELECT id, name, domain, sfin_url FROM institutions ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        Ok(Institution {
            id: r.get(0)?,
            name: r.get(1)?,
            domain: r.get(2)?,
            sfin_url: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
