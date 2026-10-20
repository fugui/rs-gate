use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 解析命令行启动参数（可选 --config / -c 指定配置文件）
    let mut config_path = None;
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if (args[i] == "--config" || args[i] == "-c") && i + 1 < args.len() {
            config_path = Some(args[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }

    // 1. 初始化结构化日志系统
    rs_gate::telemetry::init_telemetry("compact", "info")?;

    tracing::info!("Starting rs-gate (Enterprise LLM Gateway)...");

    // 2. 加载多源动态配置系统
    let config = rs_gate::config::load_config(config_path.as_deref())?;
    tracing::info!(
        "Configuration loaded successfully. Server target: {}:{}",
        config.server.host,
        config.server.port
    );

    // 3. 建立 SQLite WAL 高性能连接池
    let pool = rs_gate::storage::db::init_db_pool(&config.database).await?;
    tracing::info!("Database connection pool established with WAL mode.");

    // 4. 执行嵌入式声明式数据库自迁移
    rs_gate::storage::migration::run_migrations(&pool).await?;
    tracing::info!("Database migrations executed successfully.");

    // 5. 校验或初始化默认超级管理员账号
    let admin = rs_gate::storage::bootstrap::ensure_superadmin(
        &pool,
        config.security.default_admin_password.as_deref(),
    )
    .await?;
    tracing::info!(
        "Superadmin verified: username='{}', role='{}'",
        admin.username,
        admin.role
    );

    // 6. 启动历史审计日志滚动清理后台 Worker
    let retention_cleaner = rs_gate::audit::retention::RetentionCleaner::new(
        pool.clone(),
        config.security.audit_retention_days,
    );
    tokio::spawn(retention_cleaner.start_background_worker());
    tracing::info!(
        retention_days = config.security.audit_retention_days,
        "Audit log retention background worker started"
    );

    // 7. 组装 Axum Web 服务路由与存活探针
    let app = rs_gate::server::build_app(Some(pool)).await?;

    // 8. 绑定网络监听与服务主循环
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("rs-gate HTTP server listening on http://{}", bind_addr);

    // 9. 挂载平滑停机信号 (SIGINT / SIGTERM)
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("rs-gate server terminated gracefully.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C signal handler");
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install SIGTERM signal handler: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C (SIGINT) signal, initiating graceful shutdown...");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM signal, initiating graceful shutdown...");
        },
    }
}
