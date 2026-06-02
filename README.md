# proxy-forward

> 一个用于开发调试后端服务的本地转发工具，基于 SOCKS5 代理，使用 Rust 编写，支持按 Host + 路径前缀转发和 HTTP 路径重写。

## 目录

- [项目简介](#项目简介)
- [特性](#特性)
- [快速开始](#快速开始)
  - [编译与运行](#编译与运行)
  - [配置规则](#配置规则)
- [ZeroOmega 浏览器插件使用](#zeroomega-浏览器插件使用)
  - [安装 ZeroOmega](#安装-zeroomega)
  - [代理配置](#代理配置)
  - [规则示例](#规则示例)
- [示例场景](#示例场景)
- [项目结构](#项目结构)
- [License](#license)

## 项目简介

`proxy-forward` 是一个轻量级 SOCKS5 代理转发工具，主要面向本地开发和调试场景。

它监听本机 SOCKS5 代理端口，接收浏览器或客户端请求后，可根据配置规则：

- 匹配目标 `Host:Port`
- 匹配请求 URL 的路径前缀
- 将请求转发到指定的目标地址
- 支持将原始路径前缀重写为目标路径前缀

特别适合与浏览器代理插件（如 ZeroOmega）配合使用，实现本地请求分流、接口调试、跨域开发等。

## 特性

- 支持 SOCKS5 代理协议
- 自动生成默认 `config.toml`
- 支持按 `Host` + `path_prefix` 转发
- 支持 HTTP 请求路径重写
- 转发失败时可配置是否继续使用原始目标

## 快速开始

### 编译与运行

1. 克隆项目或进入仓库目录：

```bash
cd /run/media/serenity/iData/Serenity/project/i/rust/proxy-forward
```

2. 使用 Cargo 编译并运行：

```bash
cargo run --release
```

3. 默认监听地址：

```text
127.0.0.1:1080
```

运行时，如果当前目录下不存在 `config.toml`，程序会自动生成默认配置文件。

### 配置规则

配置文件位于项目根目录 `config.toml`，格式示例：

```toml
listen_addr = "127.0.0.1:1080"

[[rules]]
[rules.matcher]
addr = "192.168.120.177:81"
path_prefix = "/api"

[rules.forward]
addr = "127.0.0.1:8686"
path_prefix = ""
```

字段说明：

- `listen_addr`：本地 SOCKS5 监听地址
- `rules.matcher.addr`：待匹配的目标 `Host:Port`，也支持 `*` 匹配所有
- `rules.matcher.path_prefix`：需要匹配的请求路径前缀
- `rules.forward.addr`：将流量转发到的目标地址
- `rules.forward.path_prefix`：转发时替换原路径前缀的目标前缀

例如：

- 原请求 `http://192.168.120.177:81/api/users`
- 转发目标 `127.0.0.1:8686`
- 重写后路径：`http://127.0.0.1:8686/users`

## ZeroOmega 浏览器插件使用

### 安装 ZeroOmega

1. 在 Chrome/Edge 等 Chromium 浏览器中，打开扩展商店。
2. 搜索并安装 `ZeroOmega` 插件。
3. 启用插件后，进入插件的代理设置页面。

### 代理配置

在 ZeroOmega 中配置 SOCKS5 代理，指向本地 `proxy-forward` 监听地址：

- 代理类型：`SOCKS5`
- 地址：`127.0.0.1`
- 端口：`1080`

如果 ZeroOmega 允许直接填写代理字符串，也可以使用：

```text
socks5://127.0.0.1:1080
```

### 规则配置

ZeroOmega 支持根据 URL/Host 选择代理规则。推荐以下两种方式：

1. 全局代理模式

   将浏览器所有请求都走 `proxy-forward`，再由 `config.toml` 进行后端分流。

2. 分域名或自定义规则模式

   在 ZeroOmega 中只把目标调试域名或接口域名设置为走代理，其余请求直连。

示例规则：

```text
# ZeroOmega 规则示例
# 仅将目标后端接口请求走代理
http://192.168.120.177/* => SOCKS5
https://api.example.com/* => SOCKS5
```

如果你需要测试本地转发接口，可以在 ZeroOmega 中设置：

```text
http://192.168.120.177/* => SOCKS5
```

然后程序会根据 `config.toml` 中的转发规则，将请求转发到本地调试后端。

### 浏览器调试流程

1. 启动 `proxy-forward`
2. 打开 ZeroOmega，启用 `SOCKS5` 代理指向 `127.0.0.1:1080`
3. 在 ZeroOmega 中添加需要代理的 URL/Host 规则
4. 访问浏览器中的接口地址，检查是否按配置转发

## 示例场景

### 场景 1：本地 API 转发

- 浏览器访问：`http://192.168.120.177:81/api/test`
- `proxy-forward` 转发到：`http://127.0.0.1:8686/test`

### 场景 2：多域名分流

在 `config.toml` 中添加多个规则：

```toml
[[rules]]
[rules.matcher]
addr = "api.example.com:80"
path_prefix = "/service"
[rules.forward]
addr = "127.0.0.1:8080"
path_prefix = "/"

[[rules]]
[rules.matcher]
addr = "*"
path_prefix = "/static"
[rules.forward]
addr = "127.0.0.1:8090"
path_prefix = "/assets"
```

这样即可实现针对不同 Host 或路径前缀的灵活转发。

## 项目结构

- `Cargo.toml` - 项目依赖和构建配置
- `config.toml` - 默认运行时配置文件
- `src/main.rs` - 程序入口和监听逻辑
- `src/core/config.rs` - 配置加载和默认配置生成
- `src/core/socks.rs` - SOCKS5 协议处理
- `src/core/http.rs` - HTTP 解析与路径重写
- `src/core/route.rs` - 路由匹配和转发规则
- `src/libs/logs.rs` - 日志初始化

## License

本项目采用 MIT 许可证。欢迎 fork、改进和贡献。

