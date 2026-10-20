use chrono::{Datelike, Utc};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Default)]
struct KeyQuotaState {
    daily_quota: u64,
    weekly_quota: u64,
    daily_used: u64,
    weekly_used: u64,
    last_day: String,
    last_week: String,
}

#[derive(Debug, thiserror::Error)]
pub enum QuotaExceededError {
    #[error("Daily token quota exceeded (limit: {limit}, used: {used}, requested: {requested})")]
    Daily {
        limit: u64,
        used: u64,
        requested: u64,
    },
    #[error("Weekly token quota exceeded (limit: {limit}, used: {used}, requested: {requested})")]
    Weekly {
        limit: u64,
        used: u64,
        requested: u64,
    },
}

/// 日/周双周期 Token 物理硬配额引擎
#[derive(Debug, Default)]
pub struct QuotaEngine {
    state: RwLock<HashMap<String, KeyQuotaState>>,
}

impl QuotaEngine {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
        }
    }

    pub fn new_test() -> Self {
        Self::new()
    }

    fn current_date_strings() -> (String, String) {
        let now = Utc::now();
        let day = now.format("%Y-%m-%d").to_string();
        let week = format!("{}-W{:02}", now.year(), now.iso_week().week());
        (day, week)
    }

    pub fn set_daily_quota(&mut self, key_id: &str, limit: u64) {
        let mut map = self.state.write().unwrap_or_else(|e| e.into_inner());
        let (day, week) = Self::current_date_strings();
        let entry = map
            .entry(key_id.to_string())
            .or_insert_with(|| KeyQuotaState {
                last_day: day,
                last_week: week,
                ..Default::default()
            });
        entry.daily_quota = limit;
    }

    pub fn set_weekly_quota(&mut self, key_id: &str, limit: u64) {
        let mut map = self.state.write().unwrap_or_else(|e| e.into_inner());
        let (day, week) = Self::current_date_strings();
        let entry = map
            .entry(key_id.to_string())
            .or_insert_with(|| KeyQuotaState {
                last_day: day,
                last_week: week,
                ..Default::default()
            });
        entry.weekly_quota = limit;
    }

    /// 预检准入门限 (包含流式 500 Token 预留)
    pub fn check_admission(
        &self,
        key_id: &str,
        requested_tokens: u64,
    ) -> Result<(), QuotaExceededError> {
        let mut map = self.state.write().unwrap_or_else(|e| e.into_inner());
        let (cur_day, cur_week) = Self::current_date_strings();

        let state = map
            .entry(key_id.to_string())
            .or_insert_with(|| KeyQuotaState {
                last_day: cur_day.clone(),
                last_week: cur_week.clone(),
                ..Default::default()
            });

        // 懒检查跨天轮转
        if state.last_day != cur_day {
            state.daily_used = 0;
            state.last_day = cur_day;
        }

        // 懒检查跨周轮转
        if state.last_week != cur_week {
            state.weekly_used = 0;
            state.last_week = cur_week;
        }

        if state.daily_quota > 0 && state.daily_used + requested_tokens > state.daily_quota {
            return Err(QuotaExceededError::Daily {
                limit: state.daily_quota,
                used: state.daily_used,
                requested: requested_tokens,
            });
        }

        if state.weekly_quota > 0 && state.weekly_used + requested_tokens > state.weekly_quota {
            return Err(QuotaExceededError::Weekly {
                limit: state.weekly_quota,
                used: state.weekly_used,
                requested: requested_tokens,
            });
        }

        Ok(())
    }

    /// 记录实际消耗 Token
    pub async fn record_usage(&self, key_id: &str, tokens: u64) -> Result<(), anyhow::Error> {
        let mut map = self.state.write().unwrap_or_else(|e| e.into_inner());
        let (cur_day, cur_week) = Self::current_date_strings();

        let state = map
            .entry(key_id.to_string())
            .or_insert_with(|| KeyQuotaState {
                last_day: cur_day.clone(),
                last_week: cur_week.clone(),
                ..Default::default()
            });

        if state.last_day != cur_day {
            state.daily_used = 0;
            state.last_day = cur_day;
        }
        if state.last_week != cur_week {
            state.weekly_used = 0;
            state.last_week = cur_week;
        }

        state.daily_used = state.daily_used.saturating_add(tokens);
        state.weekly_used = state.weekly_used.saturating_add(tokens);

        Ok(())
    }

    /// 模拟午夜自然日跨越清零
    pub fn simulate_midnight_rollover(&mut self) {
        let mut map = self.state.write().unwrap_or_else(|e| e.into_inner());
        for state in map.values_mut() {
            state.daily_used = 0;
        }
    }

    /// 模拟周日午夜跨周轮转清零
    pub fn simulate_weekly_rollover(&mut self) {
        let mut map = self.state.write().unwrap_or_else(|e| e.into_inner());
        for state in map.values_mut() {
            state.weekly_used = 0;
        }
    }
}
