use homework_core::{
    fetch_all_homework, homework_stats_debug_report, pending_sorted_by_deadline, HomeworkItem,
    Urgency,
};
use std::env;
use std::io::{self, Write};

fn main() {
    let result = fetch_all_homework(None);
    let show_stats = env::args().any(|a| a == "--stats");
    print_reminder(&result.items);
    if result.yuketang_session_expired {
        println!("{}", "=".repeat(70));
        println!("  ⚠ 长江雨课堂登录凭证已过期，请重新扫码登录");
        println!("{}\n", "=".repeat(70));
    }
    if show_stats {
        println!("\n{}\n", homework_stats_debug_report(&result.items));
    }
}

fn print_reminder(homework_list: &[HomeworkItem]) {
    let pending = pending_sorted_by_deadline(homework_list);

    println!("\n{}", "=".repeat(70));
    println!("  作业提醒");
    println!("  待完成: {} / 总计: {}", pending.len(), homework_list.len());
    println!("{}\n", "=".repeat(70));

    if pending.is_empty() {
        println!("  没有待完成的作业！\n");
        return;
    }

    for h in pending {
        println!("  [{}] {}", h.platform, h.course);
        println!("    {}", h.title);
        println!("    截止: {}", h.deadline_display());
        if let Some(r) = h.remain_text() {
            println!("    ({r})");
        }
        if h.urgency() == Urgency::Urgent {
            println!("    ⚠ 即将截止");
        }
        println!();
    }
    let _ = io::stdout().flush();
}
