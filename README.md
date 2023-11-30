# http-relay-proxy: Record and replay your calls (ACTIVE DEVELOPMENT)

My first serious project in rust for the dev community. :D

An http proxy made with rust to record and replay your calls. It can help you for testing your apps. 

Using : 
```
Actix (with sessions)
Clap
Serde (and serde_json)
Awc
Env logger

```

## Why this project

I cant find a good solution for this job. Lightweight, Fast, Low Memory usage and support replay concurrency. I already made something with a playwright middleware (Typescript) to reproduces this behaviour but it slowdown tests and use huge amount of RAM/CPU. This project intend to help me replay API calls when running frontend e2e tests by externalizing what i did in the playwright middleware. 

I choose Rust because I did some projects using Java, JS and Go and wanted to try another language.

## Your help is welcomed

- I'm not an expert in Rust so if you have ideas to improve the actual code don't hesitate.
- If you have any request for this project create an issue.

## Tests

Actually I need to implement them. I'm doing them manually (boring).

- [ ] Passthrough
- [ ] Recording
  - [ ] Start session
  - [ ] Can make calls without issues : with/without body, handle text, json, ...
  - [ ] Stop the session
  - [ ] Calls are saved
- [ ] Replaying
  - [ ] Start session
  - [ ] Can make calls without issues : with/without body, handle text, json, ...
  - [ ] Stop the session

## Roadmap

- [ ] Add some CI
- [ ] **Improve installation flow**.
- [ ] Add testing (I'M SO BAD TO THIS NEED TO BE FIRST THING TO DO).
- [ ] When record found but a call is not present allow user to passthrough instead of raising 404.
- [ ] Use a yaml file/env vars instead of CLI options.
- [ ] Allow users to add header X-Forwarded-*;
- [ ] Allow users to replay the last identifier multiple times.
- [ ] Allow users to customize saved headers (exclude some headers, add static headers).
- [ ] Benchmark memory usage, response times
- [ ] Examples how to implement locally for playwright
- [ ] Examples how to implement using docker for playwright
- [ ] Examples how to implement in CI for playwright

## How it works

**Request are identified with : `METHOD : URL`.**

- Proxy has 3 options : passthrough, recording, replaying. The mode is selected based on options you pass to the cli.
- When running in recording or replaying you need to made 2 calls: one to start the session and one to end (otherwise memory issue ?).
- When starting a session it will add a cookie to your headers to identify your session id
- Based on this session id it will retrieve records (replay), or store them in memory (record).
- Save only happen when you stop the session. In record mode save the file with suffix .snap in JSON format.

### Passthrough

Just relay your requests nothing more

### Record

Store your request sequentially in your session. When you end the session it will save them in the file. 

### Replay

Get the file of the record. When you make a call it will retrieve sequentially the record based on the identifier. If a call is not found it will return a 404.


## How to install

I will try to improve this flow ! :')

1. Clone this repository
2. Build it : `cargo build`
3. Go in build directory : `cd target/debug`
4. Run `./http-replay-proxy` here

## How to Use

### Recording mode

1. Run the proxy : `./http-replay-proxy -f http://exemple.com -d ./snapshots/ -u`
2. Start a session in your script : `curl -X POST http://localhost:3333/start-record/test-recordname`
3. Make some calls through the proxy
4. Stop the session : `curl -X POST http://localhost:3333/end-record`
5. Calls should be saved in ./snapshots/test-recordname.snap

### Replay mode

1. Run the proxy : `./http-replay-proxy -f http://exemple.com -d ./snapshots/`
2. Start a session in your script : `curl -X POST http://localhost:3333/start-record/test-recordname`
3. Make some calls through the proxy
4. Stop the session : `curl -X POST http://localhost:3333/end-record`

## CLI usage

```
Usage: http-replay-proxy [OPTIONS] --forward-to <FORWARD_TO>

Options:
  -l, --listen-addr <LISTEN_ADDR>  [default: localhost]
  -p, --port <PORT>                [default: 3333]
  -f, --forward-to <FORWARD_TO>
  -u, --record                     Use this to update your snapshots
  -d, --dir <RECORD_DIR>           Directory where to store/to get records [default: ]
  -h, --help                       Print help
  -V, --version                    Print version
```

### Passthrough mode (just relay)

`./http-replay-proxy -f http://exemple.com`

### Record calls

`./http-replay-proxy -f http://exemple.com -d ./dir_to_save_records -u`

### Replay calls

`./http-replay-proxy -f http://exemple.com -d ./dir_to_save_records`

## Questioning

- How this is memory efficient ?
- Can I store records more efficiently (may be not human readable) to improve speed and memory usage ?
- How can I make this program as a lib to let users implements custom handlers ?
- How to make installation easy ?
