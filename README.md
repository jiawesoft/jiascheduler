# Jiascheduler

**English** · [简体中文](./README.zh-CN.md) · [Wiki](https://github.com/jiawesoft/jiascheduler/wiki/Install)

An open-source, high-performance, scalable, and dynamically configured task scheduler written in Rust that supports pushing user scripts to tens of thousands of instances simultaneously and collecting real-time execution results.

The nodes where jiascheduler executes scripts do not need to be on the same network. It has designed a sophisticated network penetration model that can manage nodes from different subnets with a single console; For example, you can https://jiascheduler.iwannay.cn Simultaneously push script execution to Tencent Cloud, Alibaba Cloud, and Amazon Cloud, and of course, you can deploy script execution to your home computer.

In order to facilitate node management, jiascheduler also provides a powerful webssh terminal that supports multi session operations, screen splitting, uploading, downloading, and more.

## Architecture

![Architecture](./assets/jiascheduler-arch.png)

## Quick start

### [💖 Jiascheduler download click here 💖 ](https://github.com/jiawesoft/jiascheduler/releases)

[https://jiascheduler.iwannay.cn](https://jiascheduler.iwannay.cn)

guest account：guest Password：guest

At this time, there are no online nodes under the guest account. You can deploy the agent yourself, and the successfully deployed agent will automatically connect to the jiascheduler online console. You can check the status of the agent, execute scripts, and view the execution results in the console.

```bash
# Only use job scheduling capability
./jiascheduler-agent --comet-addr ws://115.159.194.153:3000 --assign-username guest --assign-password guest

# Utilize job scheduling and webssh capabilities
./jiascheduler-agent --comet-addr ws://115.159.194.153:3000 --assign-username guest --assign-password guest --ssh-user your_ssh_user --ssh-port 22 --ssh-password your_ssh_user_password --namespace home
```

If you need to log off the node, simply exit the agent

### Manual compilation

1. Compile the frontend project

```bash
# Clone the repository
git clone https://github.com/jiawesoft/jiascheduler-ui.git
# Install dependencies
cd jiascheduler-ui
pnpm install
# Compile the project
pnpm build
# After compilation, copy the files from the dist directory to the dist directory of jiascheduler
cp -r dist/* jiascheduler/dist/
```

2. Compile jiascheduler

```bash
# Compile
cargo build -r --target x86_64-unknown-linux-musl
# Check the compiled executable files
ls target/x86_64-unknown-linux-musl/release
```

### Complete installation

1. Install jiascheduler-console

```bash
# Usage: jiascheduler-console [OPTIONS]

# Options:
#   -d, --debug                        if enable debug mode
#       --bind-addr <BIND_ADDR>        http server listen address, eg: "0.0.0.0:9090"
#       --config <FILE>                where to read config file, you can temporarily overwrite the configuration file using command-line parameters [default: ~/.jiascheduler/console.toml]
#   -h, --help                         Print help
#   -V, --version                      Print version

# The first installation requires specifying --bind-add. After the service starts, access 0.0.0.0:9090, enter the installation interface, and follow the prompts to complete the installation
./jiascheduler-console --bind-addr 0.0.0.0:9090
```

2. Install jiaschduler-comet

```bash
# Usage: jiascheduler-comet [OPTIONS]

# Options:
#   -d, --debug            if enable debug mode
#   -b, --bind <BIND>      [default: 0.0.0.0:3000]
#   -r <REDIS_URL>         [default: redis://:wang@127.0.0.1]
#       --secret <SECRET>  [default: rYzBYE+cXbtdMg==]
#   -h, --help             Print help
#   -V, --version          Print version

## Set the Comet listening address, and use the default value for Secret
./jiascheduler-comet --bind 0.0.0.0:3000
```

3. Install jiascheduler-agent

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


# Utilize job scheduling and webssh capabilities
# SSH related configurations can also be omitted and can be configured directly in the console later
./jiascheduler-agent --comet-addr ws://115.159.194.153:3000 --assign-username guest --assign-password guest --ssh-user your_ssh_user --ssh-port 22 --ssh-password your_ssh_user_password --namespace home

```

### Docker Deployment

Create a `.env` file in the same directory as `docker-compose.yml` with the following content:

```shell
WORKCONF=/data/jiascheduler
WORKDATA=/data/jiascheduler
```

The `console.toml` file has a default path in the container at `/root/.jiascheduler/console.toml`. If it does not exist, accessing the console page will prompt you to fill in relevant information, and it will be automatically created.

If the `console.toml` file exists, accessing the console page will directly take you to the login page. Below is a reference configuration. Save the following content as `console.toml` and place it in the `$WORKCONF/.jiascheduler` directory:

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

After executing `docker compose up -d`, access `0.0.0.0:9090`. If you are prompted with "invalid username," it means that starting via the configuration file currently does not support automatic user creation. You need to execute the following SQL to create a user. (If `console.toml` is automatically generated, it will automatically create the user.)

```sql
INSERT INTO jiascheduler.`user` (user_id,username,nickname,is_root,role_id,salt,password,avatar,email,phone,gender,introduction,created_time,updated_time) VALUES
	 ('NDoFVL5BKj','admin','admin',1,1,'FDzVZNHWWr3mPd6JBVcZD','d733f3b2c0662a4ce0c0f83cda78f7f2','','','','male','','2025-02-24 20:07:03','2025-02-24 20:07:03');

INSERT INTO jiascheduler.`role` (name,info,is_admin,created_user,created_time,updated_time) VALUES
	 ('admin','System initialization administrator role, unable to delete',1,'admin','2025-02-24 20:07:03','2025-02-24 20:07:03');
```

Below is a reference Docker configuration:

[docker-compose.yml](docker-compose.yml)

## Screenshot

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

## Help video

https://www.bilibili.com/video/BV19wzKYVEHL

## Buy me a coffee

**wechat:** cg1472580369

<img src="./assets/good.jpg" width="400px" />
