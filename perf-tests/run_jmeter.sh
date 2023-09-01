#!/bin/sh

~/jmeter/apache-jmeter-5.5/bin/jmeter --nongui --testfile plan.jmx | \
    grep "summary ="
echo
