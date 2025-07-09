#!/bin/bash

# 演示 ledgerflow-balancer 的日志输出

echo "🚀 Starting ledgerflow-balancer with enhanced logging..."
echo "📋 This demo shows the new logging features added to display program execution phases"
echo ""

# 设置日志级别
export RUST_LOG=info

# 显示日志级别说明
echo "📊 Log Level: INFO"
echo "🔍 You will see:"
echo "   - 🚀 Startup phase logs"
echo "   - 📋 Configuration loading"
echo "   - 🔗 Database connection"
echo "   - 🔄 Background task initialization"
echo "   - 🏗️ Application routes setup"
echo "   - 🌐 Server binding"
echo "   - 🎯 Service ready notifications"
echo "   - 💡 Available endpoints list"
echo "   - 🏥 Health check requests"
echo "   - 📝 API request processing"
echo "   - ⚡ Background task processing"
echo ""

# 检查配置文件是否存在
if [ ! -f "config.yaml" ]; then
    echo "⚠️  config.yaml not found, creating from example..."
    cp config.yaml.example config.yaml
fi

echo "🎬 Starting the application..."
echo "Press Ctrl+C to stop the application"
echo ""

# 运行应用程序
cargo run --bin ledgerflow-balancer -- --config config.yaml
