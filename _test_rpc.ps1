$token = 'de1d7994d60575e4c805d0ff2fad7b40ee9904cd3f0d494b226afd14acbe30be'  
$body = '{"jsonrpc":"2.0","method":"core.ping","params":{},"id":1}'  
Invoke-RestMethod -Uri http://127.0.0.1:7788/rpc -Method Post -Headers @{Authorization="Bearer $token"; "Content-Type"="application/json"} -Body $body 
