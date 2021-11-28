package main

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net/http"
	"os"
	"os/exec"
	"strconv"
	"strings"
)

const (
	Stopped      = "Stopped"
	Starting     = "Starting"
	Running      = "Running"
	ShuttingDown = "ShuttingDown"
	Updating     = "Updating"
	Unknown      = "Unknown"
)

const (
	lgsmDir = "/home/zach/.steam/steam/steamapps/common/Valheim dedicated server/lgsm/"
)

type serverList map[string]string

var servers = serverList{
	"Rotis":   "vhserver-2",
	"Default": "vhserver",
}

type response struct {
	Servers serverList `json:"servers"`
}

type serverRequest struct {
	Server string `json:"server"`
	Action string `json:"action"`
}

func (s serverList) ServeHTTP(w http.ResponseWriter, req *http.Request) {
	switch req.Method {
	case http.MethodGet:
		resp := new(response)
		resp.Servers = getServerStatuses()
		data, _ := json.Marshal(resp)
		w.Write(data)
	case http.MethodPost:
		body := new(serverRequest)
		data, err := ioutil.ReadAll(req.Body)
		if err != nil {
			fmt.Printf("error reading POST request body: %s", err.Error())
			w.WriteHeader(http.StatusInternalServerError)
			return
		}
		err = json.Unmarshal(data, body)
		if err != nil {
			fmt.Printf("error unmarshalling POST request body: %s", err.Error())
			w.WriteHeader(http.StatusBadRequest)
			return
		}
		if server, exists := s[body.Server]; exists {
			statuses := getServerStatuses()
			switch body.Action {
			case "Start":
				go startServer(server)
				statuses[server] = Starting
			case "Stop":
				go stopServer(server)
				statuses[server] = ShuttingDown
			case "Update":
				go updateServer(server)
				statuses[server] = Updating
			default:
				w.WriteHeader(http.StatusBadRequest)
			}
			resp := new(response)
			resp.Servers = statuses
			data, _ := json.Marshal(resp)
			w.Write(data)
		} else {
			w.WriteHeader(http.StatusBadRequest)
			return
		}
	default:
		w.WriteHeader(http.StatusMethodNotAllowed)
	}
}

var ip = ""

func main() {
	resp, err := http.Get("https://icanhazip.com")
	if err != nil {
		panic(err)
	}
	body, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		panic(err)
	}
	ip = strings.TrimSpace(string(body))
	http.ListenAndServe(":8085", servers)
}

func startServer(server string) error {
	cmd := exec.Command(fmt.Sprintf("%s../%s", lgsmDir, server), "start")
	return execServerCmd(cmd)
}

func updateServer(server string) error {
	cmd := exec.Command(fmt.Sprintf("%s../%s", lgsmDir, server), "update")
	return execServerCmd(cmd)
}

func execServerCmd(cmd *exec.Cmd) error {
	resp, err := cmd.Output()
	if err != nil {
		return err
	}
	if strings.HasSuffix(string(resp), "with code: 0") {
		return nil
	}
	return fmt.Errorf(string(resp))
}

func stopServer(server string) error {
	cmd := exec.Command(fmt.Sprintf("%s../%s", lgsmDir, server), "stop")
	return execServerCmd(cmd)
}

func getServerStatuses() map[string]string {
	statuses := make(map[string]string)
	for k, v := range servers {
		data, err := os.ReadFile(lgsmDir + fmt.Sprintf("lock/%s.lock", v))
		if err != nil {
			if os.IsNotExist(err) {
				statuses[k] = Stopped
				continue
			}
			panic(err)
		}
		str := string(data)
		lines := strings.Split(str, "\n")
		portString := lines[2]
		port, _ := strconv.ParseInt(portString, 10, 64)
		cmd := exec.Command("python", lgsmDir+"functions/query_gsquery.py", "-a", ip, "-p", fmt.Sprintf("%d", port+1), "-e", "protocol-valve")
		resp, err := cmd.Output()
		if err != nil {
			statuses[k] = Unknown
			continue
		}
		if strings.HasPrefix(string(resp), "OK") {
			statuses[k] = Running
		} else {
			statuses[k] = Unknown
		}
	}
	return statuses
}
