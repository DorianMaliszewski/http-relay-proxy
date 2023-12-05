## Use Docker 

You can use this tool in a docker by using a common command

### Passthrough

```
docker run --rm -p 3333:3333 maliszewskid/http_replay_proxy:latest -l 0.0.0.0 -f http://exemple.com
```

### Record

```
docker run --rm -v ./records:/records -p 3333:3333 maliszewskid/http_replay_proxy:latest -l 0.0.0.0 -f http://exemple.com -u -d /records
```

### Replay

```
docker run --rm -v ./records:/records -p 3333:3333 maliszewskid/http_replay_proxy:latest -l 0.0.0.0 -f http://exemple.com -d /records
```
