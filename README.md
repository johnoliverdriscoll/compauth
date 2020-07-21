# Compressing Authority

A scalable proof-of-concept permission system using CL accumulators that is
resistant to downgrade attacks while reducing the security boundary to a
minimal physical device.

## Usage

Start the necessary services:

```shell
$ cargo run --bin authority &
$ cargo run --bin worker &
$ cargo run --bin synchronizer &
```

Try adding a permission:

```shell
$ curl -X POST localhost:3000/permission -w "\n" -d @- << EOF
> ["tick"]
> EOF
{"nonce":8302967033790438,"actions":["tick"],"version":0}
```

After the next update window has closed (60 seconds by default), you are
able to perform the `tick` action:

```shell
$ curl -X POST localhost:3000/action -w "%{http_code}\n" -d @- << EOF
> {
>   "perm": {
>     "nonce": 8302967033790438,
>     "actions": ["tick"],
>     "version": 0
>   },
>   "action": "tick"
> }
> EOF
200
```

Update the permission with a new action:

```shell
$ curl -X PUT localhost:3000/permission -w "\n" -d @- << EOF
> {
>   "perm": {
>     "nonce": 8302967033790438,
>     "actions": ["tick"],
>     "version": 0
>   },
>   "actions": ["tock"]
> }
> EOF
{"nonce":8302967033790438,"actions":["tock"],"version":1}
```

Wait another minute for the next update and try performing the new action:

```shell
$ curl -X POST localhost:3000/action -w "%{http_code}\n" -d @- << EOF
> {
>   "perm": {
>     "nonce": 8302967033790438,
>     "actions": ["tock"],
>     "version": 1
>   },
>   "action": "tock"
> }
> EOF
200
```

You can also verify that attempting an action using the previous version of the
permission results in an 401 Unauthorized:

```shell
$ curl -X POST localhost:3000/action -w "%{http_code}\n" -d @- << EOF
> {
>   "perm": {
>     "nonce": 8302967033790438,
>     "actions": ["tick"],
>     "version": 0
>   },
>   "action": "tick"
> }
> EOF
401
```

In order to test that witness updates are correct for static elements, add
a new permission:
 
```shell
$ curl -X POST localhost:3000/permission -w "\n" -d @- << EOF
> ["tack"]
> EOF
{"nonce":3276091879824438,"actions":["tack"],"version":0}
```

Thanks to the update window, service for the `tock` permission will not be
impacted by the addition of the `tack` permission:

```shell
$ curl -X POST localhost:3000/action -w "%{http_code}\n" -d @- << EOF
> {
>   "perm": {
>     "nonce": 3276091879824438,
>     "actions": ["tock"],
>     "version": 1
>   },
>   "action": "tock"
> }
> EOF
200
```

When the next update window is closed, you can verify that both permissions are
served successfully:

```shell
$ curl -X POST localhost:3000/action -w "%{http_code}\n" -d @- << EOF
> {
>   "perm": {
>     "nonce": 3276091879824438,
>     "actions": ["tack"],
>     "version": 0
>   },
>   "action": "tack"
> }
> EOF
200
$ curl -X POST localhost:3000/action -w "%{http_code}\n" -d @- << EOF
> {
>   "perm": {
>     "nonce": 8302967033790438,
>     "actions": ["tock"],
>     "version": 1
>   },
>   "action": "tock"
> }
> EOF
200
```
