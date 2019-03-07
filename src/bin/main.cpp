//
// Copyright (c) Microsoft. All rights reserved.
// Licensed under the MIT license. See LICENSE.md file in the project root for full license information.
//

// #include <speechapi_cxx.h>
#include <stddef.h>
#include <fstream>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <dlfcn.h>
#include <unistd.h>
#include <pthread.h>
// #include <string>
// #include <stdexcept>

#if defined(__APPLE__)
#define HSS_DLL "./libns_luis.dylib"
#else
#define HSS_DLL "./libns_luis.so"
#endif

// Helper functions
class WavFileReader final
{
public:

    // Constructor that creates an input stream from a file.
    WavFileReader(const std::string& audioFileName)
    {
        if (audioFileName.empty())
            throw std::invalid_argument("Audio filename is empty");

        std::ios_base::openmode mode = std::ios_base::binary | std::ios_base::in;
        m_fs.open(audioFileName, mode);
        if (!m_fs.good())
            throw std::invalid_argument("Failed to open the specified audio file.");

        // Get audio format from the file header.
        GetFormatFromWavFile();
    }
    
    int Read(uint8_t* dataBuffer, uint32_t size)
    {
        if (m_fs.eof())
            // returns 0 to indicate that the stream reaches end.
            return 0;
        m_fs.read((char*)dataBuffer, size);
        if (!m_fs.eof() && !m_fs.good())
            // returns 0 to close the stream on read error.
            return 0;
        else
            // returns the number of bytes that have been read.
            return (int)m_fs.gcount();
    }
    
    void Close()
    {
        m_fs.close();
    }

private:
    // Defines common constants for WAV format.
    static constexpr uint16_t tagBufferSize = 4;
    static constexpr uint16_t chunkTypeBufferSize = 4;
    static constexpr uint16_t chunkSizeBufferSize = 4;

    // Get format data from a wav file.
    void GetFormatFromWavFile()
    {
        char tag[tagBufferSize];
        char chunkType[chunkTypeBufferSize];
        char chunkSizeBuffer[chunkSizeBufferSize];
        uint32_t chunkSize = 0;

        // Set to throw exceptions when reading file header.
        m_fs.exceptions(std::ifstream::failbit | std::ifstream::badbit);

        try
        {
            // Checks the RIFF tag
            m_fs.read(tag, tagBufferSize);
            if (memcmp(tag, "RIFF", tagBufferSize) != 0)
                throw std::runtime_error("Invalid file header, tag 'RIFF' is expected.");

            // The next is the RIFF chunk size, ignore now.
            m_fs.read(chunkSizeBuffer, chunkSizeBufferSize);

            // Checks the 'WAVE' tag in the wave header.
            m_fs.read(chunkType, chunkTypeBufferSize);
            if (memcmp(chunkType, "WAVE", chunkTypeBufferSize) != 0)
                throw std::runtime_error("Invalid file header, tag 'WAVE' is expected.");

            // The next chunk must be the 'fmt ' chunk.
            ReadChunkTypeAndSize(chunkType, &chunkSize);
            if (memcmp(chunkType, "fmt ", chunkTypeBufferSize) != 0)
                throw std::runtime_error("Invalid file header, tag 'fmt ' is expected.");

            // Reads format data.
            m_fs.read((char *)&m_formatHeader, sizeof(m_formatHeader));

            // Skips the rest of format data.
            if (chunkSize > sizeof(m_formatHeader))
                m_fs.seekg(chunkSize - sizeof(m_formatHeader), std::ios_base::cur);

            // The next must be the 'data' chunk.
            ReadChunkTypeAndSize(chunkType, &chunkSize);
            if (memcmp(chunkType, "data", chunkTypeBufferSize) != 0) {
                m_fs.seekg(chunkSize, std::ios_base::cur);
                ReadChunkTypeAndSize(chunkType, &chunkSize);
                if (memcmp(chunkType, "data", chunkTypeBufferSize) != 0)
                    throw std::runtime_error("Currently the 'data' chunk must directly follow the fmt chunk.");
            }
            if (m_fs.eof() && chunkSize > 0)
                throw std::runtime_error("Unexpected end of file, before any audio data can be read.");
        }
        catch (std::ifstream::failure e)
        {
            throw std::runtime_error("Unexpected end of file or error when reading audio file.");
        }
        // Set to not throw exceptions when starting to read audio data
        m_fs.exceptions(std::ifstream::goodbit);
    }

    void ReadChunkTypeAndSize(char* chunkType, uint32_t* chunkSize)
    {
        // Read the chunk type
        m_fs.read(chunkType, chunkTypeBufferSize);

        // Read the chunk size
        uint8_t chunkSizeBuffer[chunkSizeBufferSize];
        m_fs.read((char*)chunkSizeBuffer, chunkSizeBufferSize);

        // chunk size is little endian
        *chunkSize = ((uint32_t)chunkSizeBuffer[3] << 24) |
            ((uint32_t)chunkSizeBuffer[2] << 16) |
            ((uint32_t)chunkSizeBuffer[1] << 8) |
            (uint32_t)chunkSizeBuffer[0];
    }

    // The format structure expected in wav files.
    struct WAVEFORMAT
    {
        uint16_t FormatTag;        // format type.
        uint16_t Channels;         // number of channels (i.e. mono, stereo...).
        uint32_t SamplesPerSec;    // sample rate.
        uint32_t AvgBytesPerSec;   // for buffer estimation.
        uint16_t BlockAlign;       // block size of data.
        uint16_t BitsPerSample;    // Number of bits per sample of mono data.
    } m_formatHeader;
    static_assert(sizeof(m_formatHeader) == 16, "unexpected size of m_formatHeader");

private:
    std::fstream m_fs;
};


int main(int argc, char **argv)
{
    void *handle;
    typedef void *(*START_SERVICE)(const char*);
    typedef int32_t (*WRITE_STREAM)(const char *, const char *, size_t);
    char *error;
    char uuid[] = "00000000-0000-0000-0000-000000000000";
    uint8_t buf[1000];
    pthread_t thread1;
    int iret, len, i;
    START_SERVICE start_service = NULL;
    WRITE_STREAM write_stream = NULL;

    handle = dlopen(HSS_DLL, RTLD_LAZY);
    if (!handle)
    {
        fputs(dlerror(), stderr);
        exit(1);
    }

    start_service = (START_SERVICE) dlsym(handle, "start_service");
    write_stream = (WRITE_STREAM) dlsym(handle, "write_stream");
    if ((error = dlerror()) != NULL)
    {
        fputs(error, stderr);
        exit(1);
    }

    printf("start streaming ...\n");
    iret = pthread_create(&thread1, NULL, (void* (*)(void*))start_service, (void* ) "nsl.toml");
    if (iret)
    {
        fprintf(stderr, "Error - pthread_create() return code: %d\n", iret);
        exit(EXIT_FAILURE);
    }
    sleep(2);

    WavFileReader reader("chinese_test.wav");
    // Read data and push them into the stream
    int readSamples = 0;
    while((readSamples = reader.Read(buf, 640)) != 0)
    {
        // Push a buffer into the stream
        write_stream(uuid, (const char*)buf, readSamples);
        usleep(20000);
    }
    printf("try to join thread.\n");
    pthread_join(thread1, NULL);
    printf("try to close dll\n");
    dlclose(handle);
}
