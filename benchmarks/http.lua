-- wrkの計測値はmicroseconds。status errorも保存する。
init = function(args)
    if args[1] == "echo" then
        wrk.method = "POST"
        wrk.headers["Content-Type"] = "application/json"
        local size = tonumber(args[2] or "5")
        wrk.body = '{"name":"' .. string.rep("x",size) .. '","age":18}'
    end
end
done = function(s,l,r)
    io.write(string.format('RESULT {"requests":%d,"duration_us":%d,"requests_s":%.3f,"p50_us":%.3f,"p95_us":%.3f,"p99_us":%.3f,"max_us":%d,"connect_errors":%d,"read_errors":%d,"write_errors":%d,"timeouts":%d,"status_errors":%d}\n',s.requests,s.duration,s.requests/(s.duration/1000000),l:percentile(50),l:percentile(95),l:percentile(99),l.max,s.errors.connect,s.errors.read,s.errors.write,s.errors.timeout,s.errors.status))
end
