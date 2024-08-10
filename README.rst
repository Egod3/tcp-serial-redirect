tcp-serial-redirect
*********************
Overview
********

Expose a serial port over the network to be accessed with telnet.

Build and Run
***************

- To build the application run ``cargo build``::

  $ cargo build

- To execute the application run ``cargo run`` or ``target/debug/tcp-serial-redirect``::

  $ cargo run
  $ target/debug/tcp-serial-redirect

Test
****

- To test the build and run procedure was a success, you need a device that displays info over a UART/Serial Port.
- You can run Zephyr RTOS and enable the UART shell for a good test device.  It runs on most modern 32-bit micro-controllers/micro-processors::

  $ tcp-serial-redirect -s /dev/ttyACM0 -b 115200 -a 192.168.1.1 -p 2024 -ddd

- Now go to another machine and run the following::

  $ telnet 192.168.1.1 2024

- Hit enter and tab once then see::

  $ uart:~$

Updating this file
******************

- Within the python venv use ``restview README.rst`` to view this file in a browser.
