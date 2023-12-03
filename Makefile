run-passthrough:
	cargo run -- -f https://jsonplaceholder.typicode.com/ 

run-record:
	cargo run -- -f https://jsonplaceholder.typicode.com/ -d ./tmp -u

run-replay:
	cargo run -- -f https://jsonplaceholder.typicode.com/ -d ./tmp
