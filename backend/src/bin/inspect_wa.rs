use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let db_url = env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await?;

    println!("--- INSPECTING TIMETABLE & ACTIVE PERIOD ---");

    // 1. Timezone and time
    let time_info: Vec<(chrono::DateTime<chrono::Utc>, String, String)> = sqlx::query_as(
        r#"
        SELECT
            NOW() as utc_now,
            (NOW() AT TIME ZONE 'Asia/Kolkata')::text as ist_now,
            (NOW() AT TIME ZONE 'Asia/Kolkata')::time::text as ist_time
        "#,
    )
    .fetch_all(&pool)
    .await?;
    for (utc, ist, ist_time) in time_info {
        println!(
            "UTC NOW: {} | IST NOW: {} | IST TIME: {}",
            utc, ist, ist_time
        );
    }

    // 2. Teacher staff profile for Deon Aji Mathew
    let staff: Vec<(uuid::Uuid, uuid::Uuid, String)> = sqlx::query_as(
        r#"
        SELECT s.id, s.school_id, COALESCE(u.display_name, u.username) as name
        FROM staff s
        JOIN users u ON u.id = s.user_id
        WHERE u.display_name ILIKE '%Deon%' OR u.username ILIKE '%Deon%'
        "#,
    )
    .fetch_all(&pool)
    .await?;

    for (sid, school_id, name) in &staff {
        println!("Staff: {} | ID: {} | School: {}", name, sid, school_id);

        // Sync timetable for today
        let sync_res: i32 = sqlx::query_scalar(
            "SELECT sync_daily_timetable_for_date((NOW() AT TIME ZONE COALESCE((SELECT timezone FROM schools WHERE id = $1), 'Asia/Kolkata'))::date)"
        )
        .bind(school_id)
        .fetch_one(&pool)
        .await?;
        println!(
            "sync_daily_timetable_for_date result: {} rows inserted/upserted",
            sync_res
        );

        // Check count of rows in current_day_timetable for today
        let cdt_today_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM current_day_timetable WHERE school_id = $1 AND date = (NOW() AT TIME ZONE 'Asia/Kolkata')::date"
        )
        .bind(school_id)
        .fetch_one(&pool)
        .await?;
        println!(
            "current_day_timetable rows for today (2026-11-09) after sync: {}",
            cdt_today_count
        );

        // Let's test the exact check_active_period query:
        let active = sqlx::query(
            r#"
            SELECT
                cdt.class_section_id,
                c.name as class_name,
                sec.name as section_name,
                cdt.period_number,
                cdt.subject_id,
                sub.name as subject_name,
                cdt.start_time::text as start_time_str,
                cdt.end_time::text as end_time_str,
                cdt.date as cdt_date,
                (NOW() AT TIME ZONE COALESCE(sch.timezone, 'Asia/Kolkata'))::date as sch_date,
                (NOW() AT TIME ZONE COALESCE(sch.timezone, 'Asia/Kolkata'))::time as sch_time,
                cdt.start_time,
                cdt.end_time
            FROM current_day_timetable cdt
            JOIN schools sch ON sch.id = cdt.school_id
            JOIN class_sections cs ON cs.id = cdt.class_section_id
            JOIN classes c ON c.id = cs.class_id
            JOIN sections sec ON sec.id = cs.section_id
            JOIN subjects sub ON sub.id = cdt.subject_id
            WHERE cdt.school_id = $1
              AND (cdt.effective_teacher_id = $2 OR cdt.co_teacher_ids @> ARRAY[$2::uuid])
              AND cdt.date = (NOW() AT TIME ZONE COALESCE(sch.timezone, 'Asia/Kolkata'))::date
              AND (NOW() AT TIME ZONE COALESCE(sch.timezone, 'Asia/Kolkata'))::time >= (cdt.start_time - INTERVAL '10 minutes')
              AND (NOW() AT TIME ZONE COALESCE(sch.timezone, 'Asia/Kolkata'))::time <= (cdt.end_time + INTERVAL '10 minutes')
            ORDER BY cdt.period_number ASC
            LIMIT 1
            "#,
        )
        .bind(school_id)
        .bind(sid)
        .fetch_optional(&pool)
        .await?;

        if let Some(r) = active {
            let cname: String = r.get("class_name");
            let sname: String = r.get("section_name");
            let pnum: i32 = r.get("period_number");
            let subname: String = r.get("subject_name");
            let st: String = r.get("start_time_str");
            let et: String = r.get("end_time_str");
            println!(
                "FOUND ACTIVE PERIOD: P{} ({}-{}) | {} - {} | {}",
                pnum, st, et, cname, sname, subname
            );
        } else {
            println!("No active period found!");
        }
    }
    Ok(())
}
