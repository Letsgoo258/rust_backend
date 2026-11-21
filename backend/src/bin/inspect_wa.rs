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
            "current_day_timetable rows for today after sync: {}",
            cdt_today_count
        );
    }

    // 3. Check across the ENTIRE school for ANY clashing slots in timetable_slots
    println!("\n--- Checking ENTIRE SCHOOL for clashing timetable slots ---");
    let all_clashing_master: Vec<(String, i32, i32, i64, Vec<String>)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(u.display_name, u.username) as teacher_name,
            ts.day_of_week,
            ts.period_number,
            COUNT(*) as slot_count,
            array_agg(c.name || ' ' || sec.name || ' (' || sub.name || ')') as classes
        FROM timetable_slots ts
        JOIN staff st ON st.id = ts.teacher_staff_id
        JOIN users u ON u.id = st.user_id
        JOIN class_sections cs ON cs.id = ts.class_section_id
        JOIN classes c ON c.id = cs.class_id
        JOIN sections sec ON sec.id = cs.section_id
        JOIN subjects sub ON sub.id = ts.subject_id
        WHERE ts.status = 'ACTIVE'
        GROUP BY u.display_name, u.username, ts.day_of_week, ts.period_number
        HAVING COUNT(*) > 1
        ORDER BY teacher_name, ts.day_of_week, ts.period_number
        "#,
    )
    .fetch_all(&pool)
    .await?;

    println!(
        "Total teacher clashes across entire school in master timetable: {}",
        all_clashing_master.len()
    );
    for (name, dow, p, count, classes) in &all_clashing_master {
        println!(
            "  ⚠️ Clash: Teacher '{}' on Day {} Period {} has {} slots: {:?}",
            name, dow, p, count, classes
        );
    }

    // 4. Test new clean attendance options query for Deon on 2026-11-20
    println!("\n--- Testing clean attendance options query for Deon (2026-11-20) ---");
    let test_options = sqlx::query(
        r#"
        SELECT * FROM (
            -- 1. Designated Class Teacher homeroom / daily roll call
            SELECT
                cs.id AS class_section_id,
                c.name AS class_name,
                sec.name AS section_name,
                c.sequence AS class_seq,
                NULL::uuid AS subject_id,
                'Daily Roll Call' AS subject_name,
                NULL::int AS period_number,
                true AS is_daily_roll_call
            FROM class_sections cs
            JOIN classes c ON c.id = cs.class_id
            JOIN sections sec ON sec.id = cs.section_id
            WHERE c.school_id = $1
              AND (cs.class_teacher_staff_id = $2 OR cs.class_teacher_staff_id = $3)

            UNION

            -- 2. Today's Timetable slots (including daily substitutions)
            SELECT
                cs.id AS class_section_id,
                c.name AS class_name,
                sec.name AS section_name,
                c.sequence AS class_seq,
                sub.id AS subject_id,
                sub.name AS subject_name,
                cdt.period_number,
                false AS is_daily_roll_call
            FROM current_day_timetable cdt
            JOIN schools sch ON sch.id = cdt.school_id
            JOIN class_sections cs ON cs.id = cdt.class_section_id
            JOIN classes c ON c.id = cs.class_id
            JOIN sections sec ON sec.id = cs.section_id
            JOIN subjects sub ON sub.id = cdt.subject_id
            WHERE cdt.school_id = $1
              AND (cdt.effective_teacher_id = $2 OR cdt.effective_teacher_id = $3 
                   OR cdt.co_teacher_ids @> ARRAY[$2::uuid] OR cdt.co_teacher_ids @> ARRAY[$3::uuid])
              AND cdt.date = $4
        ) AS t
        ORDER BY is_daily_roll_call DESC, period_number ASC NULLS FIRST, class_seq ASC, section_name ASC, subject_name ASC
        "#
    )
    .bind(staff[0].1)
    .bind(staff[0].0)
    .bind(staff[0].0) // user_id or staff_id
    .bind(chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap())
    .fetch_all(&pool)
    .await?;

    println!(
        "Total clean options for Friday (2026-11-20): {}",
        test_options.len()
    );
    for r in &test_options {
        let is_daily: bool = r.get("is_daily_roll_call");
        let c: String = r.get("class_name");
        let s: String = r.get("section_name");
        let sub: String = r.get("subject_name");
        let p: Option<i32> = r.try_get("period_number").ok();
        if is_daily {
            println!("  • {} {} | 📋 Daily Roll Call", c, s);
        } else {
            println!("  • {} {} | 📚 {} (Period {})", c, s, sub, p.unwrap_or(0));
        }
    }
    Ok(())
}
