import json
from http.server import BaseHTTPRequestHandler, HTTPServer

import requests


class MBTAProxyHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        print(f"received HTTP request for path: {self.path}")
        outgoing_url = f"https://api-v3.mbta.com{self.path}"

        try:
            response = requests.get(outgoing_url)
            response.raise_for_status()

            self.send_response(response.status_code)
            self.send_header("Content-type", "application/json")
            self.end_headers()

            print(f"MBTA response: {response.content}")
            arrival_time = response.json()["data"][0]["attributes"]["arrival_time"]
            data = {"datetime": arrival_time}
            self.wfile.write(json.dumps(data).encode("utf-8"))
            print("done\n\n")

        except requests.exceptions.RequestException as e:
            self.send_error(500, f"Error fetching from MBTA API: {e}")


def run(server_class=HTTPServer, handler_class=MBTAProxyHandler, port=80):
    server_address = ("", port)
    httpd = server_class(server_address, handler_class)
    print(f"Starting server on port {port}...")
    httpd.serve_forever()


if __name__ == "__main__":
    run()
