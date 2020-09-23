delete from waves where wave_name = $1::text returning wave_id;
