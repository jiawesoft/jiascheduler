# jiascheduler

**简体中文** · [English](./README.zh-CN.md)

一个用rust编写的开源高性能，可扩展，动态配置的任务调度器，支持同时推送用户脚本到数以万计的实例运行，并实时收集执行的结果。

jiascheduler 执行脚本的节点不需要都在同一个网络，其内部设计了一个精巧的网络穿透模型可以用一个控制台管理不同子网的节点；举例，你可以在 https://jiascheduler.iwannay.cn 同时往腾讯云， 阿里云，亚马逊云推送脚本执行，当然你可以往家里的电脑部署脚本执行。

为了方便对节点进行管理，jiascheduler同时提供了一个功能强大的webssh终端，支持多会话操作，分屏，上传，下载等。


## 架构图

![架构图](./assets/jiascheduler-arch.png)

## 快速开始

[https://jiascheduler.iwannay.cn](https://jiascheduler.iwannay.cn) 
访客账号：guest 密码：guest

此时guest账号下并没有在线的节点，你可以自己部署Agent，部署成功的Agent将自动接入jiascheduler在线控制台，你可以在控制台查看Agent的状态，执行脚本，查看执行结果。

```bash
# 仅使用作业调度能力
./jiascheduler-agent --comet-addr ws://115.159.194.153:3000 --assign-username guest --assign-password guest

# 使用作业调度能力和webssh能力
./jiascheduler-agent --comet-addr ws://115.159.194.153:3000 --assign-username guest --assign-password guest --ssh-user your_ssh_user --ssh-port 22 --ssh-password your_ssh_user_password --namespace home
```

如果你需要下线节点，只需要退出Agent即可

**完整安装**

1. 安装jiascheduler-console
```bash
# Usage: jiascheduler-console [OPTIONS]

# Options:
#   -d, --debug                        if enable debug mode
#       --bind-addr <BIND_ADDR>        http server listen address, eg: "0.0.0.0:9090"
#       --config <FILE>                where to read config file, you can temporarily overwrite the configuration file using command-line parameters [default: ~/.jiascheduler/console.toml]
#   -h, --help                         Print help
#   -V, --version                      Print version

# 首次安装需要指定--bind-addr，服务启动后访问0.0.0.0:9090，进入安装界面，按提示完成安装
./jiascheduler-console --bind-addr 0.0.0.0:9090
```



2. 安装jiaschduler-comet
```bash
# Usage: jiascheduler-comet [OPTIONS]

# Options:
#   -d, --debug            if enable debug mode
#   -b, --bind <BIND>      [default: 0.0.0.0:3000]
#   -r <REDIS_URL>         [default: redis://:wang@127.0.0.1]
#       --secret <SECRET>  [default: rYzBYE+cXbtdMg==]
#   -h, --help             Print help
#   -V, --version          Print version

# 设置comet监听地址，secret则采用默认值
./jiascheduler-comet --bind 0.0.0.0:3000
```

3. 安装jiascheduler-agent
```bash
# Usage: jiascheduler-agent [OPTIONS]

# Options:
#   -d, --debug
#           If enable debug mode
#   -b, --bind <BIND>
#           [default: 0.0.0.0:3001]
#       --comet-addr <COMET_ADDR>
#           [default: ws://127.0.0.1:3000]
#       --output-dir <OUTPUT_DIR>
#           Directory for saving job execution logs [default: ./log]
#       --comet-secret <COMET_SECRET>
#           [default: rYzBYE+cXbtdMg==]
#   -n, --namespace <NAMESPACE>
#           [default: default]
#       --ssh-user <SSH_USER>
#           Set the login user of the instance for SSH remote connection
#       --ssh-password <SSH_PASSWORD>
#           Set the login user's password of the instance for SSH remote connection
#       --ssh-port <SSH_PORT>
#           Set the port of this instance for SSH remote connection
#       --assign-username <ASSIGN_USERNAME>
#           Assign this instance to a user and specify their username
#       --assign-password <ASSIGN_PASSWORD>
#           Assign this instance to a user and specify their password
#   -h, --help
#           Print help
#   -V, --version
#           Print version


# 使用作业调度能力和webssh能力
# ssh相关配置也可以不传，稍后可以在控制台直接配置
./jiascheduler-agent --comet-addr ws://115.159.194.153:3000 --assign-username guest --assign-password guest --ssh-user your_ssh_user --ssh-port 22 --ssh-password your_ssh_user_password --namespace home

```



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


## 赞助
