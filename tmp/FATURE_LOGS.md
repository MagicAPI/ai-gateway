What we want is this ->

For each requeest/response we add a new metrics middleware.
This basically calcualts these metrics for each requst and then can send to a logs sink like ElasticSearch, ccloidwatch or other sinks.

Metrics:
- latency
- request size
- response size
- status code
- error count
- error types
- provider name
- provider latency
- provider status code
- provider error count
- provider error types
- provider request size
- provider response size
<Please think wisely and only choose the metrics that are most useful and not too much to compute.>

Other metadatata:
- provider
- mode name
- inout token
- outout token
- total token
- cost if availble othe rmetrics.

Most of the requests are /chat/compeltiong and hence you need to get some of these metrics fromt he json repsonse of the steraming or non-streaming response.
Now plan and create various code for this.

Also allow "debug" mode to print these metrics to the console..
