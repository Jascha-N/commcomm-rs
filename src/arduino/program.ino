#include <stdint.h>

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wignored-qualifiers"
#include <EEPROM.h>
#pragma GCC diagnostic pop

#include <ArduinoJson.h>
#include <RingBufCPP.h>


#ifndef TXLED0
#define TXLED0
#endif

#ifndef TXLED1
#define TXLED1
#endif

#ifndef RXLED0
#define RXLED0
#endif

#ifndef RXLED1
#define RXLED1
#endif


struct SensorPins {
    uint8_t analog;
    uint8_t digital;
};

struct SensorThresholds {
    uint8_t trigger;
    uint8_t release;
};

struct SensorCalibration {
    int low;
    int high;
};

struct SensorState {
    bool flexed;
    int raw;
    uint8_t mapped;
};

enum class EventType {
    SENSOR_FLEXED,
    SENSOR_EXTENDED,
    MODE_CHANGED
};

enum class Mode {
    COMMAND,
    CALIBRATION_FLEXED,
    CALIBRATION_EXTENDED,
    CALIBRATION_FINAL
};

struct Event {
    EventType type;
    union {
        uint8_t sensor_id;
        Mode mode;
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


static const SensorPins sensor_pins[] = SENSOR_PINS;

static const size_t num_sensors = sizeof(sensor_pins) / sizeof(sensor_pins[0]);

// Number of events stored in the event queue
static const size_t event_queue_size = 10;

// Maximum request/response length including null-terminator
static const size_t message_buffer_size = 256;

// Buffer size for JSON
static const size_t json_buffer_size = 1024;

// Serial settings
static const unsigned long baudrate = 115200;

static const RawJson json_null = RawJson("null");


static volatile Mode mode;

// Queue with events
static RingBufCPP<Event, event_queue_size> events;

// Previous state of the sensors
static SensorState sensor_state[num_sensors];

// Sensor thresholds
static SensorThresholds sensor_thresholds[num_sensors];

// Sensor limits
static SensorCalibration sensor_calibration[num_sensors];


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
    StaticJsonBuffer<json_buffer_size> json_buffer;

    const JsonObject& json_request = json_buffer.parseObject(buffer);
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

        json_response.printTo(buffer, message_buffer_size);
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

        if (json_response.measureLength() > message_buffer_size - 1) {
            return CommandResult::ERROR_BUFFER_TOO_SMALL;
        }

        json_response.printTo(buffer, message_buffer_size);
    } else if (strcmp(command, "set_thresholds") == 0) {
        uint8_t id = json_request.get<uint8_t>("id");
        if (id >= num_sensors) {
            return CommandResult::ERROR_INVALID_PARAM;
        }
        SensorThresholds &thresholds = sensor_thresholds[id];
        thresholds.trigger = json_request.get<uint8_t>("trigger");
        thresholds.release = json_request.get<uint8_t>("release");

        return CommandResult::SUCCESS_NULL;
    } else if (strcmp(command, "read_values") == 0) {
        bool raw = json_request.get<bool>("raw");

        JsonArray& json_response = json_buffer.createArray();
        if (!json_response.success()) {
            return CommandResult::ERROR_JSON_ALLOC;
        }
        for (uint8_t id = 0; id < num_sensors; id++) {
            const SensorState& state = sensor_state[id];

            if (!json_response.add(raw ? state.raw : static_cast<int>(state.mapped))) {
                return CommandResult::ERROR_JSON_ALLOC;
            }
        }

        if (json_response.measureLength() > message_buffer_size - 1) {
            return CommandResult::ERROR_BUFFER_TOO_SMALL;
        }

        json_response.printTo(buffer, message_buffer_size);
    } else {
        return CommandResult::ERROR_UNKNOWN_COMMAND;
    }

    return CommandResult::SUCCESS;
}

static void process_inputs() {
    for (uint8_t id = 0; id < num_sensors; id++) {
        const SensorThresholds& thresholds = sensor_thresholds[id];
        const SensorCalibration& calibration = sensor_calibration[id];
        SensorState& state = sensor_state[id];

        state.raw = analogRead(sensor_pins[id].analog);
        state.mapped = constrain(map(state.raw, calibration.low, calibration.high, 0, UINT8_MAX), 0, UINT8_MAX);
        bool new_event = false;
        Event event;

        if (state.mapped > thresholds.trigger && !state.flexed) {
            event = {EventType::SENSOR_FLEXED, id};
            state.flexed = true;
            new_event = true;
        } else if (state.mapped < thresholds.release && state.flexed) {
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

void calibration_isr() {
    switch (mode) {
        case Mode::COMMAND:
        {
            static unsigned long LAST_TIME = 0;
            unsigned long time = millis();

            if (LAST_TIME != 0 && time - LAST_TIME < 1000) {
                // Event event;
                // event.type = EventType::MODE_CHANGED;
                // event.mode = mode;
                // events.add(event);
                mode = Mode::CALIBRATION_FLEXED;
            }
            LAST_TIME = time;
            break;
        }
        case Mode::CALIBRATION_FLEXED:
        {
            mode = Mode::CALIBRATION_EXTENDED;
            break;
        }
        case Mode::CALIBRATION_EXTENDED:
        {
            mode = Mode::CALIBRATION_FINAL;
            break;
        }
        default:
            break;
    }
}

void setup() {
    mode = Mode::COMMAND;

    EEPROM.get(0, sensor_calibration);

    for (uint8_t id = 0; id < num_sensors; id++) {
        pinMode(sensor_pins[id].digital, INPUT);
    }

    Serial.begin(baudrate);

#ifdef CALIBRATION_PIN
    pinMode(CALIBRATION_PIN, INPUT);
    attachInterrupt(digitalPinToInterrupt(CALIBRATION_PIN), calibration_isr, RISING);
    interrupts();
#endif
}

void loop() {
    switch (mode) {
        case Mode::CALIBRATION_FLEXED:
        {
            TXLED1;
            RXLED0;

            for (uint8_t id = 0; id < num_sensors; id++) {
                sensor_calibration[id].high = analogRead(sensor_pins[id].analog);
                delay(1);
            }

            break;
        }
        case Mode::CALIBRATION_EXTENDED:
        {
            TXLED0;
            RXLED1;

            for (uint8_t id = 0; id < num_sensors; id++) {
                sensor_calibration[id].low = analogRead(sensor_pins[id].analog);
                delay(1);
            }

            break;
        }
        case Mode::CALIBRATION_FINAL:
        {
            for (int i = 0; i < 3; i++) {
                TXLED1;
                RXLED1;
                delay(100);
                TXLED0;
                RXLED0;
                delay(300);
            }

            EEPROM.put(0, sensor_calibration);
            delay(300);
            mode = Mode::COMMAND;
        }
        case Mode::COMMAND:
        {
            if (!Serial) {
                break;
            }

            process_inputs();

            // No command received, do nothing
            if (Serial.available() == 0) {
                break;
            }

            char buffer[message_buffer_size];
            size_t length = Serial.readBytesUntil('\n', buffer, message_buffer_size);
            if (length >= message_buffer_size) {
                // Flush input buffer
                while (Serial.available() > 0) {
                    Serial.read();
                }
                Serial.println(static_cast<int>(CommandResult::ERROR_REQUEST_TOO_LONG));
                Serial.flush();
                break;
            }
            buffer[length] = '\0';

            CommandResult result = process_request(buffer);
            switch (result) {
                case CommandResult::SUCCESS:
                    Serial.println(buffer);
                    break;
                case CommandResult::SUCCESS_NULL:
                    Serial.println(json_null);
                    break;
                default:
                    Serial.println(static_cast<int>(result));
            }
            Serial.flush();
        }
    }
}
