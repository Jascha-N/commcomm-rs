#include <ArduinoJson.h>
#include <RingBufCPP.h>


struct SensorConfig {
    bool active; // Whether this input is used with a flex sensor
    int min;        // Minimum usable value of the sensor based on resistance used [0, 1023]
    int max;        // Maximum usable value of the sensor based on resistance used [0, 1023]
    uint8_t low;       // Lower threshold (detrigger)
    uint8_t high;      // Upper threshold (trigger)
};

struct SensorState {
    bool flexed;
    int raw;
};

enum class EventType {
    SENSOR_FLEXED,
    SENSOR_EXTENDED
    /* ... */
};

struct Event {
    EventType type;
    union {
        uint8_t sensor_id;
        /* ... */
    };
};

enum class CommandResult {
    SUCCESS = -2,
    SUCCESS_NULL = -1,
    ERROR_JSON_PARSE = 0,
    ERROR_JSON_ALLOC,
    ERROR_REQUEST_TOO_LONG,
    ERROR_UNKNOWN_COMMAND,
    ERROR_BUFFER_TOO_SMALL,
    ERROR_INVALID_PARAM
};


// The button pins and labels
static const uint8_t NUM_SENSORS = NUM_ANALOG_INPUTS;

// Number of events stored in the event queue
static const size_t EVENT_QUEUE_SIZE = 10;

// Maximum request/response length including null-terminator
static const size_t MESSAGE_BUFFER_SIZE = 256;

// Buffer size for JSON
static const size_t JSON_BUFFER_SIZE = 1024;

// Serial settings
static const unsigned long BAUDRATE = 115200;

static const char JSON_NULL[] = "null";


// Queue with events
static RingBufCPP<Event, EVENT_QUEUE_SIZE> events;

// Previous state of the sensors
static SensorState sensor_state[NUM_SENSORS];

// Sensor settings
static SensorConfig sensor_config[NUM_SENSORS];


// Handle a JSON command. In case of an error, all commands can return an error message as a
// JSON string.
//
// Commands:
// * command: version_info
//   parameters: none
//   response: object with version (optional string) and hash (optional string)
//   description: returns software version info for version checking and debugging purposes
//
// * command: poll_events
//   parameters: none
//   response: array of objects of the form {"type": id} where type is the event type and id is the sensor id (integer)
//   description: returns all events that happened since the last poll_events command
//
// * command: set_sensor
//   parameters: id (integer), min (integer), max (integer), low (integer) and high (integer)
//   response: null
//   description: activate a sensor on the given analog pin (id) with the provided settings
//
// * command: unset_sensor
//   parameters: id (integer)
//   response: null
//   description: deactivate a sensor on the given analog pin (id)
//
// * command: raw_values
//   parameters: none
//   response: array of raw sensor values (integer, null for inactive sensors)
//   description: returns the raw sensor values for active sensors
static CommandResult process_request(char *buffer) {
    StaticJsonBuffer<JSON_BUFFER_SIZE> json_buffer;

    JsonObject& json_request = json_buffer.parseObject(buffer);
    if (!json_request.success()) {
        return CommandResult::ERROR_JSON_PARSE;
    }

    const char *command = json_request.get<const char *>("command");
    if (strcmp(command, "device_info") == 0) {
#if defined(DEVICEINFO_NAME) && defined(DEVICEINFO_VERSION) && defined(DEVICEINFO_TIMESTAMP)
        JsonObject& json_response = json_buffer.createObject();
        if (!json_response.success()) {
            return CommandResult::ERROR_JSON_ALLOC;
        }

        if (!json_response.set("name", DEVICEINFO_NAME)) {
            return CommandResult::ERROR_JSON_ALLOC;
        }
        if (!json_response.set("version", DEVICEINFO_VERSION)) {
            return CommandResult::ERROR_JSON_ALLOC;
        }
        if (!json_response.set("timestamp", DEVICEINFO_TIMESTAMP)) {
            return CommandResult::ERROR_JSON_ALLOC;
        }

        json_response.printTo(buffer, MESSAGE_BUFFER_SIZE);
#else
        return CommandResult::SUCCESS_NULL;
#endif
    } else if (strcmp(command, "poll_event") == 0) {
        Event event;
        if (!events.pull(&event)) {
            return CommandResult::SUCCESS_NULL;
        }

        JsonObject& json_response = json_buffer.createObject();
        if (!json_response.success()) {
            return CommandResult::ERROR_JSON_ALLOC;
        }

        const char *type = event.type == EventType::SENSOR_FLEXED ? "flexed" : "extended";
        if (!json_response.set(type, event.sensor_id)) {
            return CommandResult::ERROR_JSON_ALLOC;
        }

        if (json_response.measureLength() > MESSAGE_BUFFER_SIZE - 1) {
            return CommandResult::ERROR_BUFFER_TOO_SMALL;
        }

        json_response.printTo(buffer, MESSAGE_BUFFER_SIZE);
    } else if (strcmp(command, "set_sensor") == 0) {
        uint8_t id = json_request.get<uint8_t>("id");
        if (id >= NUM_SENSORS) {
            return CommandResult::ERROR_INVALID_PARAM;
        }
        SensorConfig &sensor = sensor_config[id];
        sensor.min = json_request.get<int>("min");
        sensor.max = json_request.get<int>("max");
        sensor.low = json_request.get<uint8_t>("low");
        sensor.high = json_request.get<uint8_t>("high");
        sensor.active = true;

        return CommandResult::SUCCESS_NULL;
    } else if (strcmp(command, "unset_sensor") == 0) {
        uint8_t id = json_request.get<uint8_t>("id");
        if (id >= NUM_SENSORS) {
            return CommandResult::ERROR_INVALID_PARAM;
        }
        SensorConfig &sensor = sensor_config[id];
        sensor.active = false;

        return CommandResult::SUCCESS_NULL;
    } else if (strcmp(command, "raw_values") == 0) {
        JsonArray& json_response = json_buffer.createArray();
        if (!json_response.success()) {
            return CommandResult::ERROR_JSON_ALLOC;
        }
        for (uint8_t id = 0; id < NUM_SENSORS; id++) {
            SensorConfig& config = sensor_config[id];
            SensorState& state = sensor_state[id];

            if (config.active) {
                if (!json_response.add(state.raw)) {
                    return CommandResult::ERROR_JSON_ALLOC;
                }
            } else {
                if (!json_response.add(RawJson(JSON_NULL))) {
                    return CommandResult::ERROR_JSON_ALLOC;
                }
            }
        }

        if (json_response.measureLength() > MESSAGE_BUFFER_SIZE - 1) {
            return CommandResult::ERROR_BUFFER_TOO_SMALL;
        }

        json_response.printTo(buffer, MESSAGE_BUFFER_SIZE);
    } else {
        return CommandResult::ERROR_UNKNOWN_COMMAND;
    }

    return CommandResult::SUCCESS;
}

static void process_inputs() {
    for (uint8_t id = 0; id < NUM_SENSORS; id++) {
        SensorConfig& config = sensor_config[id];
        SensorState& state = sensor_state[id];

        if (!config.active) {
            continue;
        }

        state.raw = analogRead(id);
        // Limit sensor input to usable range and then map to [0, 255] range
        uint8_t input = map(constrain(state.raw, config.min, config.max),
                            config.min, config.max, 0, 255);
        bool new_event = false;
        Event event;

        if (input > config.high && !state.flexed) {
            event = {EventType::SENSOR_FLEXED, id};
            state.flexed = true;
            new_event = true;
        } else if (input < config.low && state.flexed) {
            event = {EventType::SENSOR_EXTENDED, id};
            state.flexed = false;
            new_event = true;
        }

        if (new_event) {
            // If queue is full, remove oldest event
            while (!events.add(event)) {
                Event dummy;
                events.pull(&dummy);
            }
        }
    }
}

void setup() {
    for (uint8_t id = 0; id < NUM_SENSORS; id++) {
        pinMode(A0 + id, INPUT);
    }
    Serial.begin(BAUDRATE);
}

void loop() {
    // Do nothing while there is no connection
    if (!Serial) {
        while (!Serial) {}

        // Clear event queue and reset state and disable sensors when a new connection is established
        Event dummy;
        while (events.pull(&dummy)) {}

        for (uint8_t id = 0; id < NUM_SENSORS; id++) {
            sensor_state[id] = {false, 0};
            sensor_config[id].active = false;
        }
    }

    process_inputs();

    // No command received, do nothing
    if (Serial.available() == 0) {
        return;
    }

    char buffer[MESSAGE_BUFFER_SIZE];
    size_t length = Serial.readBytesUntil('\n', buffer, MESSAGE_BUFFER_SIZE);
    if (length >= MESSAGE_BUFFER_SIZE) {
        // Flush input buffer
        while (Serial.available() > 0) {
            Serial.read();
        }
        Serial.println(static_cast<int>(CommandResult::ERROR_REQUEST_TOO_LONG));
        Serial.flush();
        return;
    }
    buffer[length] = '\0';

    CommandResult result = process_request(buffer);
    switch (result) {
        case CommandResult::SUCCESS:
            Serial.println(buffer);
            break;
        case CommandResult::SUCCESS_NULL:
            Serial.println(JSON_NULL);
            break;
        default:
            Serial.println(static_cast<int>(result));
    }
    Serial.flush();
}
