platform select remote-ios --sysroot {sysroot}
target create "{disk_app}"
script fruitstrap_device_app="{device_app}"
script fruitstrap_connect_url="connect://127.0.0.1:{device_port}"
#script fruitstrap_output_path="{output_path}"
#script fruitstrap_error_path="{error_path}"
#target modules search-paths add {modules_search_paths_pairs}
command script import "{python_file_path}"
command script add -f {python_command}.connect_command connect
command script add -s asynchronous -f {python_command}.run_command run
command script add -s asynchronous -f {python_command}.autoexit_command autoexit
command script add -s asynchronous -f {python_command}.safequit_command safequit
connect
run
autoexit
