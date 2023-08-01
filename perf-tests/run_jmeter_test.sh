cd ~/jmeter/apache-jmeter-5.5/bin/
results=$(./jmeter -n -t ~/go-app-enclave/Test\ Plan.jmx | grep "summary =")
echo -e "$results\n"