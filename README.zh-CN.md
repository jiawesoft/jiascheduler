# jiascheduler

**简体中文** · [English](./README.md) · [Wiki](https://github.com/jiawesoft/jiascheduler/wiki/Install)

一款用 rust 编写的开源高性能，可扩展，支持动态配置的任务调度器，能够同时推送用户脚本到数以万计的实例运行，并实时收集执行的结果。

jiascheduler 执行脚本的节点不需要都在同一个网络，其内部设计了一个精巧的网络穿透模型可以用一个控制台管理不同子网的节点；例如，你可以在 https://jiascheduler.iwannay.cn 同时往腾讯云， 阿里云，亚马逊云推送脚本执行，当然你可以往家里的电脑部署脚本执行。

为了方便对节点进行管理，jiascheduler 同时提供了一个功能强大的 webssh 终端，支持多会话操作，分屏，上传，下载等。

Github 地址：https://github.com/jiawesoft/jiascheduler

## 架构图

![架构图](./assets/jiascheduler-arch.png)

## 快速开始

### [💖 jiascheduler 下载点击这里 💖 ](https://github.com/jiawesoft/jiascheduler/releases)

[https://jiascheduler.iwannay.cn](https://jiascheduler.iwannay.cn)
访客账号：guest 密码：guest

除了使用演示地址中的测试服务器，你也可以自己部署 Agent，部署成功的 Agent 将自动接入 jiascheduler 在线控制台，你可以通过控制台查看 Agent 的状态，执行脚本，查看执行结果，发起 ssh 连接。

### 接入在线控制台

以下演示了如何将自己本地实例接入 jiaschduler 在线控制台

```bash
# 仅使用作业调度能力
./jiascheduler-agent --comet-addr ws://115.159.194.153:3000 --assign-username guest --assign-password guest

# 使用作业调度能力和webssh能力
./jiascheduler-agent --comet-addr ws://115.159.194.153:3000 --assign-username guest --assign-password guest --ssh-user your_ssh_user --ssh-port 22 --ssh-password your_ssh_user_password --namespace home
```

如果你需要下线节点，只需要退出 Agent 即可

### 单实例部署

jiascheduler 一共有四个执行程序，分别是

1. jiascheduler-console: 控制台服务，用于提供 web 控制台服务
2. jiascheduler-comet: 连接层服务，用于提供一个统一的接入层供 agent 连接
3. jiascheduler-agent: 本地 agent 层序，用于执行作业
4. jiascheduler: 以上三个服务的合并打包版本，用于单节点简易快速部署。
   需要注意的是，jiascheduler 打包服务也支持不同的 agent 接入。
   部署了 jiascheduler 合并版本, 你依旧可以再部署多个 comet 和 agent

单实例部署时仅需要执行以下内容

```bash
// 通过浏览器访问 localhost:9090, 完成初始化安装
// 初始化安装后会加载配置文件再次启动无需传入--bind-addr
// 生成的配置文件路径默认为$HOME/.jiascheduler/console.toml
./jiascheduler --bind-addr 0.0.0.0:9090

```

### docker 部署

在 docker-compose.yml 同目录下创建.env 文件，内容如下

```shell
WORKCONF=/data/jiascheduler
WORKDATA=/data/jiascheduler
```

`console.toml` 在容器中默认路径为`/root/.jiascheduler/console.toml`，如果没有该配置文件，则访问 console 页面时会进入初始化安装页面

如果存在 `console.toml` 文件，访问 console 页面则直接跳到登录页面，参考配置如下，将以下内容保存为 `console.toml`，放`$WORKCONF/.jiascheduler` 目录下

```yml
debug = false
bind_addr = "0.0.0.0:9090"
api_url = ""
redis_url = "redis://default:3DGiuazc7wkAppV3@redis"
comet_secret = "rYzBYE+cXbtdMg=="
database_url = "mysql://root:kytHmeBR4Vg@mysql:3306/jiascheduler"

[encrypt]
private_key = "QGr0LLnFFt7mBFrfol2gy"

[admin]
username = "admin"
password = "qTQhiMiLCb"
```

执行 docker compose up -d 后访问 0.0.0.0:9090 进入控制台界面

docker 参考配置如下

[docker-compose.yml](docker-compose.yml)

## 软件截图

<table style="border-collapse: collapse; border: 1px solid black;">
  <tr>
    <td style="padding: 5px;background-color:#fff;"><img src= "./assets/job-edit.png" alt="Jiascheduler job edit"   /></td>
    <td style="padding: 5px;background-color:#fff;"><img src= "./assets/run-list.png" alt="Jiascheduler run list"   /></td>
  </tr>

  <tr>
    <td style="padding: 5px;background-color:#fff;"><img src= "./assets/scheduler-history.png" alt="Jiascheduler scheduler history"   /></td>
    <td style="padding: 5px;background-color:#fff;"><img src= "./assets/scheduler-dashboard.png" alt="Jiascheduler scheduler dashboard"   /></td>
  </tr>

  <tr>
    <td style="padding: 5px;background-color:#fff;"><img src= "./assets/server.png" alt="Jiascheduler server"   /></td>
    <td style="padding: 5px;background-color:#fff;"><img src= "./assets/webssh.png" alt="Jiascheduler webssh"   /></td>
  </tr>

</table>

## 帮助视频

https://www.bilibili.com/video/BV19wzKYVEHL

## 赞助

**wechat:** cg1472580369

<img src="./assets/good.jpg" width="350px" />
